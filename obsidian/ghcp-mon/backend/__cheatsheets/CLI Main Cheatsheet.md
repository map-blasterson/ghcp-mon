---
type: cheatsheet
---
Source: `src/main.rs` (binary crate `ghcp-mon`). The library it consumes is `ghcp_mon::*`.

CLI surface and entrypoint. Depends on [[Server Router Cheatsheet]] (`AppState`, `serve`), [[DB Module Cheatsheet]] (`db::open`), [[Broadcaster Cheatsheet]], and [[Ingest Pipeline Cheatsheet]] (`ingest_jsonl_file`).

## Extract

```rust
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use ghcp_mon::{db, server::{self, AppState}, ws::Broadcaster};

#[derive(Parser, Debug)]
#[command(name = "ghcp-mon", version, about = "Local-first GitHub Copilot CLI telemetry collector + dashboard backend")]
struct Cli {
    #[arg(long, global = true, default_value = "./data/ghcp-mon.db")]
    db: PathBuf,

    /// Override base directory for per-conversation `workspace.yaml` sidecars.
    #[arg(long, global = true)]
    session_state_dir: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Serve {
        #[arg(long, default_value = "127.0.0.1:4318")]
        otlp_addr: SocketAddr,
        #[arg(long, default_value = "127.0.0.1:4319")]
        api_addr: SocketAddr,
    },
    Replay {
        path: PathBuf,
        #[arg(long)] inline: bool,
        #[arg(long, default_value = "http://127.0.0.1:4319")]
        server: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> { ... }
```

Tracing setup (in `main`, before `Cli::parse`):
- Builds a `tracing_subscriber::registry()` with `EnvFilter::try_from_default_env()` falling back to `EnvFilter::new("info,sqlx=warn,tower_http=warn,hyper=warn")`, plus `fmt::layer().with_target(false)`. Initialized via `.init()`.

Replay subcommand:
- `inline = true` → opens DB, builds `AppState`, calls `ghcp_mon::ingest::ingest_jsonl_file(&state, &path, "replay-inline").await?`, prints `"ingested {n} envelopes inline from {path}"` to stdout.
- `inline = false` → canonicalizes `path`, computes `format!("{server}/api/replay")` (trimming trailing `/`), POSTs `{"path": <abs>}` JSON to that URL using `reqwest::Client::new()`, then prints `"POST {url} -> {status}: {text}"`.

`session_state_dir_override` is wrapped in `Arc::new(cli.session_state_dir.clone())` and passed into `AppState`.

## Suggested Test Strategy

`main.rs` is a binary; the recommended approach is to test the `Cli`/`Cmd` parser and shell out for behavior tests:

- **Parser tests**: re-declare or `pub`-expose `Cli` from a small testable shim (the current source has it private), or use `assert_cmd` (not in dev-deps; would need to be added). Pragmatic alternative: write an integration test under `tests/` that runs the compiled binary via `std::process::Command` (`env!("CARGO_BIN_EXE_ghcp-mon")` works in integration tests).

- LLR-aligned cases:
  - **CLI defines serve and replay subcommands**: `<bin> serve --help` and `<bin> replay --help` exit 0 and emit expected text. `<bin> nope` should fail with non-zero exit and a clap "unrecognized" error.
  - **CLI db option default path**: `<bin> --help` mentions `./data/ghcp-mon.db`. Or run `<bin> serve --otlp-addr 127.0.0.1:0 --api-addr 127.0.0.1:0` against a temp dir with cwd = tmp; stop quickly and assert that `./data/ghcp-mon.db` was created relative to cwd.
  - **CLI session-state-dir flag overrides default**: easier to check in [[Local Session Cheatsheet]] tests by directly invoking `resolve_session_state_dir(Some(path))`.
  - **CLI initializes tracing subscriber**: hard to introspect from tests. Acceptable approach: run the binary with `RUST_LOG=debug` and observe stderr contains a tracing-formatted line. Or skip with a comment: tracing init is a one-time global side effect.
  - **Serve binds OTLP and API listeners**: spawn the binary with two ephemeral ports (you can use `0:0` only via custom builds; otherwise pick free ports yourself by binding-then-releasing). Then `TcpStream::connect` should succeed on both addresses within ~2s. Kill the child on cleanup.
  - **Replay inline mode ingests in-process**: write a JSONL fixture, run `<bin> --db <tmp.db> replay <fixture> --inline`, assert exit 0 and stdout contains `"ingested <n> envelopes inline"`. Then open the same `<tmp.db>` with `db::open` and verify `raw_records` row count.
  - **Replay non-inline posts to running server**: spin up a server child first (or use a hand-rolled `axum` server in the test using `api_router(state)` from [[Server Router Cheatsheet]]), then run `<bin> replay <fixture> --server http://127.0.0.1:<port>`. Assert exit 0 and that the in-process server's `raw_records` table grew. The CLI uses `reqwest::Client::new()` over rustls (no protocol surprises).
- For most of these, you'll want `#[tokio::test(flavor = "multi_thread")]`.

Don't try to mock `clap` or `reqwest`. The binary is a thin orchestration layer — run it as a process or refactor the inner logic if more granular testing is needed.
