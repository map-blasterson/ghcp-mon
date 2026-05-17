use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use ghcp_mon::{db, export, server::{self, AppState}, ws::Broadcaster};

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=warn,hyper=warn")))
        // Diagnostics MUST go to stderr so `ghcp-mon export ... | ghcp-mon
        // replay /dev/stdin` (and any other stdout-consuming pipe) sees a
        // clean JSON-lines stream.
        .with(fmt::layer().with_target(false).with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();
    let session_state_dir_override = Arc::new(cli.session_state_dir.clone());

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
    }
    Ok(())
}
