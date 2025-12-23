mod config;
mod error;
mod handlers;
mod skills;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use handlers::{
    check_trigger, continue_factory, create_skill, delete_skill, download_file, exec_command,
    execute_code, execute_script, get_skill, health_check, list_files, list_skills, read_file,
    sandbox_info, search_skills, start_factory, stream_command, update_skill, upload_file,
    write_file,
};
use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sandbox_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(config);

    let app = Router::new()
        // Health
        .route("/health", get(health_check))
        .route("/sandbox/info", get(sandbox_info))
        // Shell
        .route("/shell/exec", post(exec_command))
        .route("/shell/stream", post(stream_command))
        // Code
        .route("/code/execute", post(execute_code))
        // Files
        .route("/file/read", get(read_file))
        .route("/file/write", post(write_file))
        .route("/file/list", get(list_files))
        .route("/file/upload", post(upload_file))
        .route("/file/download", get(download_file))
        // Skills routes
        .route("/skills", get(list_skills).post(create_skill))
        .route("/skills/search", get(search_skills))
        .route(
            "/skills/:name",
            get(get_skill).put(update_skill).delete(delete_skill),
        )
        .route("/skills/:name/scripts/:script", post(execute_script))
        // Factory routes
        .route("/factory/start", post(start_factory))
        .route("/factory/continue", post(continue_factory))
        .route("/factory/check", post(check_trigger))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
