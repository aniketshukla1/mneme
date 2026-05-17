//! # mneme-server
//!
//! Thin host process. Will expose the memory layer over MCP so any agent
//! framework (LangGraph, CrewAI, custom) can use it — see CLAUDE.md, Phase 5.
//!
//! Today it just boots the event log and prints a status line, so the
//! workspace has a runnable `cargo run` target from day one.

use mneme_store::FjallEventLog;
use mneme_core::EventLog;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let data_dir = std::env::var("MNEME_DATA").unwrap_or_else(|_| "./mneme-data".into());
    tracing::info!(%data_dir, "booting mneme");

    let log = FjallEventLog::open(&data_dir)?;
    let entries = log.read_from(None).await?;
    tracing::info!(event_count = entries.len(), "event log opened");

    println!("mneme is up. {} events in the log.", entries.len());
    println!("next: implement the vector + bm25 views (Phase 0). see CLAUDE.md.");
    Ok(())
}
