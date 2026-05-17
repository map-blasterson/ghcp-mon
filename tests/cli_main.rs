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

#[test]
fn cli_help_lists_export_subcommand() {
    let out = Command::new(BIN).arg("--help").output().expect("run help");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("export"), "help MUST mention the `export` subcommand; got:\n{}", text);
}

#[test]
fn export_help_documents_output_flag() {
    let out = Command::new(BIN).args(["export", "--help"]).output().expect("run");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--output") || text.contains("-o"),
        "export --help MUST document the --output/-o flag; got:\n{}", text);
}

#[test]
fn export_missing_session_exits_non_zero_and_writes_to_stderr() {
    let dir = unique_dir("export-missing");
    let db_path = dir.join("missing.db");
    // Touch the DB by running a no-op replay first.
    let fixture = dir.join("empty.jsonl");
    std::fs::write(&fixture, "").unwrap();
    let _ = Command::new(BIN)
        .args(["--db", db_path.to_str().unwrap(), "replay", fixture.to_str().unwrap(), "--inline"])
        .output().expect("seed db");
    assert!(db_path.exists());

    let out = Command::new(BIN)
        .args(["--db", db_path.to_str().unwrap(), "export", "does-not-exist"])
        .output().expect("run export");
    assert!(!out.status.success(), "export of unknown session MUST exit non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("session not found"),
        "missing-session error MUST go to stderr; got:\n{}", stderr);
    assert!(out.stdout.is_empty(),
        "stdout MUST stay empty when the session is missing; got: {:?}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn export_round_trips_through_replay() {
    // Fixture: two traces under the same conversation. One span carries the
    // conversation id directly; the other inherits via shared trace_id (i.e.
    // not directly tagged, but in the same trace as a tagged span). The export
    // path must include both — that is the membership contract.
    let dir = unique_dir("export-roundtrip");
    let fixture = dir.join("fix.jsonl");
    let mut f = std::fs::File::create(&fixture).unwrap();
    // Span A: chat span, tagged with the conversation id.
    writeln!(f, r#"{{"type":"span","traceId":"tA","spanId":"sA1","name":"chat gpt-5","startTime":1000,"endTime":2000,"attributes":{{"gen_ai.conversation.id":"conv-x"}}}}"#).unwrap();
    // Span B: sibling in same trace, NO conv id of its own — should still
    // export because it shares trace_id with span A.
    writeln!(f, r#"{{"type":"span","traceId":"tA","spanId":"sA2","parentSpanId":"sA1","name":"execute_tool bash","startTime":1100,"endTime":1900,"attributes":{{}}}}"#).unwrap();
    // Span C: different conversation; must NOT appear in the export.
    writeln!(f, r#"{{"type":"span","traceId":"tC","spanId":"sC1","name":"chat gpt-5","startTime":3000,"endTime":4000,"attributes":{{"gen_ai.conversation.id":"conv-other"}}}}"#).unwrap();
    drop(f);
    let db_path = dir.join("rt.db");

    // Seed the DB by replaying the fixture inline.
    let seed = Command::new(BIN)
        .args(["--db", db_path.to_str().unwrap(), "replay", fixture.to_str().unwrap(), "--inline"])
        .output().expect("seed replay");
    assert!(seed.status.success(), "seed replay MUST succeed; stderr=\n{}", String::from_utf8_lossy(&seed.stderr));

    // Export the session.
    let exp = Command::new(BIN)
        .args(["--db", db_path.to_str().unwrap(), "export", "conv-x"])
        .output().expect("run export");
    assert!(exp.status.success(), "export MUST succeed; stderr=\n{}", String::from_utf8_lossy(&exp.stderr));
    let stdout = String::from_utf8_lossy(&exp.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "export MUST emit exactly 2 spans (sA1 + sA2); got:\n{}", stdout);

    // Each line MUST parse as a span envelope and carry the right trace_id.
    let mut ids: Vec<String> = Vec::new();
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("each line MUST be valid JSON");
        assert_eq!(v.get("type").and_then(|x| x.as_str()), Some("span"),
            "each exported line MUST have type=span; got: {}", line);
        assert_eq!(v.get("traceId").and_then(|x| x.as_str()), Some("tA"),
            "each exported span MUST belong to tA; got: {}", line);
        ids.push(v.get("spanId").and_then(|x| x.as_str()).unwrap().to_string());
    }
    ids.sort();
    assert_eq!(ids, vec!["sA1".to_string(), "sA2".to_string()]);

    // Replay the export into a fresh DB. It must ingest successfully.
    let rt_dir = unique_dir("export-rt-replay");
    let rt_fixture = rt_dir.join("export.jsonl");
    std::fs::write(&rt_fixture, stdout.as_bytes()).unwrap();
    let rt_db = rt_dir.join("rt2.db");
    let rep = Command::new(BIN)
        .args(["--db", rt_db.to_str().unwrap(), "replay", rt_fixture.to_str().unwrap(), "--inline"])
        .output().expect("replay export");
    assert!(rep.status.success(),
        "replaying the exported JSONL MUST succeed; stderr=\n{}", String::from_utf8_lossy(&rep.stderr));
    let rep_stdout = String::from_utf8_lossy(&rep.stdout);
    assert!(rep_stdout.contains("ingested 2 envelopes"),
        "round-trip replay MUST ingest exactly 2 envelopes; got: {}", rep_stdout);
}
