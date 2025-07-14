//! REST API implementation

mod handlers;
mod websocket;

use crate::{process::ProcessManager, logs::{LogStorage, LogSearchEngine}};
use axum::{
    Router,
    routing::{get, post, delete},
    Extension,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub use handlers::create_process;

pub fn create_router(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    search_engine: Option<Arc<LogSearchEngine>>,
) -> Router {
    let mut router = Router::new()
        // Process management
        .route("/api/processes", post(handlers::create_process))
        .route("/api/processes", get(handlers::list_processes))
        .route("/api/processes/clean", post(handlers::clean_processes))
        .route("/api/processes/:id", get(handlers::get_process))
        .route("/api/processes/:id", delete(handlers::stop_process))
        .route("/api/processes/:id/restart", post(handlers::restart_process))
        .route("/api/processes/:id/health", get(handlers::get_process_health))
        
        // Log access
        .route("/api/logs/:id", get(handlers::get_logs))
        .route("/api/logs/:id/stream", get(websocket::stream_logs))
        
        // Agent endpoints
        .route("/api/agent/query", post(handlers::agent_query))
        .route("/api/agent/summary", get(handlers::agent_summary))
        .route("/api/agent/query-schema", get(handlers::get_query_schema))
        .route("/api/agent/capabilities", get(handlers::get_capabilities))
        
        // Human endpoints
        .route("/api/logs/:id/raw", get(handlers::get_raw_logs))
        .route("/api/attach/:id", get(websocket::attach_to_process))
        
        // Health check
        .route("/health", get(handlers::health_check))
        
        // Add shared state
        .layer(Extension(process_manager))
        .layer(Extension(log_storage));

    // Add search route and engine only if search engine is available
    if let Some(engine) = search_engine {
        router = router
            .route("/api/logs/search", post(handlers::search_logs))
            .layer(Extension(engine));
    }
    
    router.layer(CorsLayer::permissive())
}