mod config;
mod errors;
mod routes;
mod agent_client;
mod wol;

use std::sync::Arc;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // journald tracing and stderr with env filter
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(env_filter);

    if let Ok(journald) = tracing_journald::layer() {
        registry
            .with(journald)
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
    } else {
        registry
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
    }

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/wakecomputer/config.yaml".to_string());

    let config = match config::load_config(&config_path) {
        Ok(c) => {
            tracing::info!(
                path = config_path,
                machines = c.machines.len(),
                "configuration loaded"
            );
            c
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to load configuration");
            std::process::exit(1);
        }
    };

    let listen = config
        .listen
        .clone()
        .unwrap_or_else(|| "127.0.0.1:9876".to_string());

    let app = routes::app(Arc::new(config));

    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(address = listen, error = %e, "failed to bind");
            std::process::exit(1);
        });

    tracing::info!(address = listen, "server started");

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    });
}
