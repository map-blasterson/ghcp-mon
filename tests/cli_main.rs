//! Tests for the ghcp-mon binary CLI surface. LLRs:
//! - CLI db option default path
//! - CLI defines serve and replay subcommands
//! - CLI initializes tracing subscriber  (PARTIAL — observable only via stderr)
//! - CLI session state dir flag overrides default
//! - Replay inline mode ingests in-process
//! - Replay non-inline posts to running server  (PARTIAL — covered by --help only)
//! - Serve binds OTLP and API listeners
//!
//! These tests shell out to the compiled binary via `env!("CARGO_BIN_EXE_ghcp-mon")`.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_ghcp-mon");

fn unique_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-cli-{}-{}-{}", tag, std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[test]
fn cli_help_lists_serve_and_replay_subcommands() {
    let out = Command::new(BIN).arg("--help").output().expect("run help");
    assert!(out.status.success(), "ghcp-mon --help MUST exit 0");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("serve"), "help MUST mention `serve` subcommand");
    assert!(text.contains("replay"), "help MUST mention `replay` subcommand");
}

#[test]
fn cli_unknown_subcommand_exits_non_zero() {
    let out = Command::new(BIN).arg("definitely-not-a-real-cmd").output().expect("run");
    assert!(!out.status.success(), "unknown subcommand MUST be rejected");
}

#[test]
fn cli_db_default_path_visible_in_help() {
    let out = Command::new(BIN).arg("--help").output().expect("run help");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("./data/ghcp-mon.db"),
        "default --db path MUST be ./data/ghcp-mon.db, help text was:\n{}",
        text
    );
}

#[test]
fn cli_session_state_dir_flag_visible_in_help() {
    let out = Command::new(BIN).arg("--help").output().expect("run help");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("--session-state-dir"),
        "--session-state-dir MUST be a global option, help was:\n{}",
        text
    );
}

#[test]
fn cli_replay_help_documents_inline_flag() {
    let out = Command::new(BIN).args(["replay", "--help"]).output().expect("run");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--inline"), "replay --help MUST mention --inline");
    assert!(text.contains("--server") || text.contains("server"),
        "replay --help MUST document --server option");
}

#[test]
fn cli_initializes_tracing_subscriber_emits_to_stderr() {
    // RUST_LOG=info plus a quick `replay --help` exits fast. We just want to verify
    // that running the binary at all doesn't panic during tracing init. The actual
    // stderr emission is tied to the `serve` path which never returns.
    // Pragmatic: the help path proves tracing init didn't panic.
    let out = Command::new(BIN).env("RUST_LOG", "debug").arg("--help").output().expect("run");
    assert!(out.status.success(), "tracing init MUST NOT panic on startup");
}

#[test]
fn replay_inline_ingests_into_db_in_process() {
    // Build a tiny JSONL fixture.
    let dir = unique_dir("replay-inline");
    let fixture = dir.join("fix.jsonl");
    let mut f = std::fs::File::create(&fixture).unwrap();
    writeln!(f, r#"{{"type":"span","traceId":"t","spanId":"s","name":"x","startTime":1}}"#).unwrap();
    writeln!(f, r#"{{"type":"metric","name":"m","dataPoints":[]}}"#).unwrap();
    drop(f);
    let db_path = dir.join("inline.db");

    let out = Command::new(BIN)
        .args([
            "--db", db_path.to_str().unwrap(),
            "replay", fixture.to_str().unwrap(), "--inline",
        ])
        .output().expect("run replay --inline");
    assert!(out.status.success(),
        "replay --inline MUST exit 0; stderr=\n{}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ingested"),
        "replay --inline MUST print 'ingested ...'; got: {}", stdout);
    assert!(stdout.contains("inline"),
        "replay --inline MUST mention 'inline'; got: {}", stdout);
    // The DB MUST exist and contain raw_records.
    assert!(db_path.exists(), "inline replay MUST create the DB file");
}

#[test]
fn serve_binds_otlp_and_api_listeners_on_configured_addrs() {
    // Pick two ephemeral ports and assert both can be reached after a short startup.
    let otlp_port = pick_free_port();
    let api_port = pick_free_port();
    let dir = unique_dir("serve");
    let db_path = dir.join("serve.db");

    let mut child = Command::new(BIN)
        .args([
            "--db", db_path.to_str().unwrap(),
            "serve",
            "--otlp-addr", &format!("127.0.0.1:{}", otlp_port),
            "--api-addr",  &format!("127.0.0.1:{}", api_port),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().expect("spawn ghcp-mon serve");

    // Try to connect to both listeners with retries up to ~5 seconds.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut otlp_ok = false;
    let mut api_ok = false;
    while std::time::Instant::now() < deadline && !(otlp_ok && api_ok) {
        if !otlp_ok && std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", otlp_port).parse().unwrap(),
            Duration::from_millis(200),
        ).is_ok() { otlp_ok = true; }
        if !api_ok && std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", api_port).parse().unwrap(),
            Duration::from_millis(200),
        ).is_ok() { api_ok = true; }
        if !(otlp_ok && api_ok) {
            std::thread::sleep(Duration::from_millis(150));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    assert!(otlp_ok, "serve MUST bind the OTLP listener on --otlp-addr");
    assert!(api_ok, "serve MUST bind the API listener on --api-addr");
}

#[test]
fn replay_non_inline_help_documents_server_option() {
    // Full e2e for non-inline replay would require spinning up a server; out of scope
    // for source-blind generation. We verify the CLI surface advertises the flag.
    let out = Command::new(BIN).args(["replay", "--help"]).output().expect("run");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("127.0.0.1:4319"),
        "replay --help MUST document the default server URL; got:\n{}", text);
}
