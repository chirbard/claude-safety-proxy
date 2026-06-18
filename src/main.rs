mod config;
mod extractor;
mod forwarder;
mod proxy;
mod scanner;

use std::sync::Arc;
use axum::{Router, routing::any};
use tracing::info;

use config::Config;
use proxy::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "claude_safety_proxy=debug,warn".parse().unwrap()),
        )
        .init();

    let config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("Could not load config.toml ({e}), using defaults");
        Config::default()
    });

    info!("Proxy listening on :{}", config.port);
    info!("Ollama: {} model={}", config.ollama_url, config.model);

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .build()?;

    let state = Arc::new(AppState { config: config.clone(), client });

    let app = Router::new()
        .route("/*path", any(proxy::handle))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
