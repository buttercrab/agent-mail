mod config;
mod domain;
mod error;
mod http;
mod id;
mod mcp;
mod store;
mod time;
mod validation;

use anyhow::Context;
use config::Config;
use store::Store;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "agent_mail_server=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env();
    let store = Store::connect(&config.database_url)
        .await
        .context("connect to postgres and migrate schema")?;
    let app = http::router(http::AppState {
        store,
        token: config.token,
        allowed_origins: config.allowed_origins,
        mcp: mcp::McpHub::default(),
    })
    .layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("bind {}", config.bind))?;
    tracing::info!(addr = %config.bind, "agent-mail rust server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
