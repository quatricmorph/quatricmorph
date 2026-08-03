//! `q-daemon` — the local HTTP service binary.
//!
//! Data plane: serves the **Metadata Plane** and exact **Artifact Plane** reads
//! (ARCHITECTURE.md §2.1, §14).
//!
//! Every model root is ingested at startup — headers and shard index only — so
//! startup time is independent of checkpoint size. No payload byte is read
//! until a request asks for a specific scalar or block.

use clap::Parser;
use q_daemon::{router, AppState, DaemonConfig};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "q-daemon",
    about = "Quatricmorph local API over SafeTensors checkpoints"
)]
struct Cli {
    /// Directory holding a checkpoint. Repeat for several models.
    ///
    /// This is the security boundary: nothing outside a configured root is
    /// readable, and no request may name a filesystem path (`SEC-001`).
    #[arg(long = "model-root", value_name = "DIR", required = true)]
    model_roots: Vec<PathBuf>,

    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "q_daemon=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let mut config = DaemonConfig::new(&cli.bind);
    for root in &cli.model_roots {
        let label = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| root.display().to_string());
        info!(root = %root.display(), "adding model root");
        config = config.with_root(label, root)?;
    }

    let state = AppState::bootstrap(config)?;
    for id in state.model_ids() {
        if let Some(m) = state.catalog().get_model(&id)? {
            let unresolved = state.catalog().unresolved_count(&id)?;
            info!(
                model_id = %m.model_id,
                source = %m.source_key,
                resolver = %m.resolver_id,
                tensors = m.tensor_count,
                parameters = m.parameter_count,
                unresolved_tensors = unresolved,
                "model ready (metadata only; no weights loaded)"
            );
        }
    }

    let listener = tokio::net::TcpListener::bind(&cli.bind).await?;
    info!(address = %cli.bind, "listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
