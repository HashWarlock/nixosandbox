mod config;
mod error;
mod handlers;
mod state;

use std::net::SocketAddr;
use axum::{Router, routing::get};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use state::AppState;
use handlers::{health_check, sandbox_info};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
