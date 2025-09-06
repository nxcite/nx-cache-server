pub mod handlers;
pub mod middleware;
pub mod error;
pub mod validation;

use axum::{
    routing::{get, put},
    Router,
    middleware as axum_middleware,
};
use std::sync::Arc;
use crate::domain::{config::ServerConfig, storage::StorageProvider};

#[derive(Clone)]
pub struct AppState<T: StorageProvider> {
    pub storage: Arc<T>,
    pub config: Arc<ServerConfig>,
}

pub fn create_router<T: StorageProvider + Clone>() -> Router<AppState<T>> {
    let protected_routes = Router::new()
        .route("/v1/cache/:hash", get(handlers::retrieve_artifact::<T>))
        .route("/v1/cache/:hash", put(handlers::store_artifact::<T>))
        .layer(axum_middleware::from_fn(middleware::auth_middleware));

    // Combine public and protected routes
    Router::new()
        .route("/health", get(handlers::health_check))
        .merge(protected_routes)
}

pub async fn run_server<T: StorageProvider + Clone>(
    storage: T, 
    config: &ServerConfig
) -> Result<(), std::io::Error> {
    let app_state = AppState {
        storage: Arc::new(storage),
        config: Arc::new(config.clone()),
    };
    
    let app = create_router::<T>().with_state(app_state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    
    tracing::info!("Server running on port {}", config.port);
    axum::serve(listener, app).await?;
    
    Ok(())
}