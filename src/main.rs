use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use ghcp_mon::{db, export, server::{self, AppState}, ws::Broadcaster, tui};

#[derive(Parser, Debug)]
#[command(name = "ghcp-mon", version, about = "Local-first GitHub Copilot CLI telemetry collector + dashboard backend")]
struct Cli {
    /// SQLite DB file path. Defaults to ./data/ghcp-mon.db
    #[arg(long, global = true, default_value = "./data/ghcp-mon.db")]
    db: PathBuf,

    /// Override base directory for per-conversation `workspace.yaml` sidecars.
    /// Takes precedence over `$COPILOT_SESSION_STATE_DIR` and `$HOME/.copilot/session-state`.
    #[arg(long, global = true)]
    session_state_dir: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Start the HTTP server (OTLP receiver + REST API + WebSocket).
    Serve {
        /// Address for OTLP/HTTP receiver (POST /v1/traces|metrics|logs)
        #[arg(long, default_value = "127.0.0.1:4318")]
        otlp_addr: SocketAddr,
        /// Address for the dashboard REST API + WebSocket
        #[arg(long, default_value = "127.0.0.1:4319")]
        api_addr: SocketAddr,
    },
    /// Replay a JSON-lines telemetry file (file-exporter format).
    Replay {
        /// Path to a `.log` / `.jsonl` file
        path: PathBuf,
        /// If set, ingest in-process instead of POSTing to a running server.
        #[arg(long)]
        inline: bool,
        /// Server URL to POST replay requests to (when not --inline)
        #[arg(long, default_value = "http://127.0.0.1:4319")]
        server: String,
    },
    /// Export a session's spans as replay-compatible JSON-lines.
    ///
    /// Emits one span envelope per line in the file-exporter format. The
    /// output can be fed back through `ghcp-mon replay` to reconstitute the
    /// session in a fresh database. Metrics and logs are not included.
    Export {
        /// `gen_ai.conversation.id` of the session to export.
        session: String,
        /// Optional output file. Defaults to stdout.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Attach a terminal UI to a running `ghcp-mon serve`.
    Attach {
        /// Server base URL. Trailing `/` is stripped. Only http/https accepted.
        #[arg(long, default_value = "http://127.0.0.1:4319")]
        server: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let session_state_dir_override = Arc::new(cli.session_state_dir.clone());

    // Tracing init is split by subcommand: `Attach` needs a rolling-file +
    // in-process-buffer sink (writing to stderr would corrupt the alternate
    // screen); every other subcommand keeps the historical stderr fmt layer.
    let filter = || {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=warn,hyper=warn"))
    };
    let log_buffer_opt = if matches!(cli.cmd, Cmd::Attach { .. }) {
        let log_buffer = tui::LogBuffer::new();
        let log_path = tui::persist::log_path()
            .ok_or_else(|| anyhow::anyhow!("no cache dir available for TUI log file"))?;
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let dir = log_path.parent().unwrap().to_path_buf();
        let name = log_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("tui.log"))
            .to_string_lossy()
            .to_string();
        let appender = tracing_appender::rolling::never(dir, name);
        let (file_writer, guard) = tracing_appender::non_blocking(appender);
        // Leak the guard so the writer flushes for the program's lifetime.
        Box::leak(Box::new(guard));
        tracing_subscriber::registry()
            .with(filter())
            .with(
                fmt::layer()
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(file_writer),
            )
            .with(tui::widgets::log_overlay::LogBufferLayer::new(
                log_buffer.clone(),
            ))
            .init();
        Some(log_buffer)
    } else {
        tracing_subscriber::registry()
            .with(filter())
            // Diagnostics MUST go to stderr so `ghcp-mon export ... | ghcp-mon
            // replay /dev/stdin` (and any other stdout-consuming pipe) sees a
            // clean JSON-lines stream.
            .with(fmt::layer().with_target(false).with_writer(std::io::stderr))
            .init();
        None
    };

    match cli.cmd {
        Cmd::Serve { otlp_addr, api_addr } => {
            let pool = db::open(&cli.db).await?;
            let bus = Broadcaster::new(1024);
            let state = AppState { pool, bus, session_state_dir_override };
            server::serve(state, otlp_addr, api_addr).await?;
        }
        Cmd::Replay { path, inline, server: server_url } => {
            if inline {
                let pool = db::open(&cli.db).await?;
                let bus = Broadcaster::new(1024);
                let state = AppState { pool, bus, session_state_dir_override };
                let n = ghcp_mon::ingest::ingest_jsonl_file(&state, &path, "replay-inline").await?;
                println!("ingested {n} envelopes inline from {}", path.display());
            } else {
                let abs = std::fs::canonicalize(&path)?;
                let url = format!("{}/api/replay", server_url.trim_end_matches('/'));
                let body = serde_json::json!({"path": abs.to_string_lossy()});
                let resp = reqwest::Client::new().post(&url).json(&body).send().await?;
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                println!("POST {url} -> {status}: {text}");
            }
        }
        Cmd::Export { session, output } => {
            let pool = db::open(&cli.db).await?;
            // Preflight: a missing session must NOT create/truncate an
            // output file.
            if !export::session_exists(&pool, &session).await? {
                eprintln!("session not found: {session}");
                std::process::exit(1);
            }
            let count = match output {
                Some(path) => {
                    let f = tokio::fs::File::create(&path).await?;
                    let mut w = tokio::io::BufWriter::new(f);
                    let n = export::export_session(&pool, &session, &mut w).await?;
                    tokio::io::AsyncWriteExt::flush(&mut w).await?;
                    eprintln!("exported {n} spans to {}", path.display());
                    n
                }
                None => {
                    let mut w = tokio::io::BufWriter::new(tokio::io::stdout());
                    let n = export::export_session(&pool, &session, &mut w).await?;
                    tokio::io::AsyncWriteExt::flush(&mut w).await?;
                    eprintln!("exported {n} spans");
                    n
                }
            };
            let _ = count;
        }
        Cmd::Attach { server } => {
            let log_buffer = log_buffer_opt.expect("log buffer initialized for Attach");
            tui::run(&server, log_buffer).await?;
        }
    }
    Ok(())
}
