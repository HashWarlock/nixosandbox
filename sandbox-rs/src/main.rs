mod browser;
mod config;
mod error;
mod handlers;
mod skills;
mod state;

#[cfg(feature = "tee")]
mod tee;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use handlers::{
    browser_click, browser_evaluate, browser_goto, browser_screenshot, browser_status,
    browser_type, check_trigger, continue_factory, create_skill, delete_skill, download_file,
    exec_command, execute_code, execute_script, get_skill, health_check, list_files, list_skills,
    read_file, sandbox_info, search_skills, start_factory, stream_command, update_skill,
    upload_file, write_file,
};

#[cfg(feature = "tee")]
use handlers::tee::{
    derive_key, emit_event, generate_quote, sign_data, tee_info, verify_signature,
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
            "/skills/{name}",
            get(get_skill).put(update_skill).delete(delete_skill),
        )
        .route("/skills/{name}/scripts/{script}", post(execute_script))
        // Factory routes
        .route("/factory/start", post(start_factory))
        .route("/factory/continue", post(continue_factory))
        .route("/factory/check", post(check_trigger))
        // Browser routes
        .route("/browser/goto", post(browser_goto))
        .route("/browser/screenshot", post(browser_screenshot))
        .route("/browser/evaluate", post(browser_evaluate))
        .route("/browser/click", post(browser_click))
        .route("/browser/type", post(browser_type))
        .route("/browser/status", get(browser_status));

    #[cfg(feature = "tee")]
    let app = app
        .route("/tee/info", get(tee_info))
        .route("/tee/quote", post(generate_quote))
        .route("/tee/derive-key", post(derive_key))
        .route("/tee/sign", post(sign_data))
        .route("/tee/verify", post(verify_signature))
        .route("/tee/emit-event", post(emit_event));

    let app = app.with_state(state).layer(TraceLayer::new_for_http());

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
