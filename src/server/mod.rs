pub mod error;
pub mod handlers;
pub mod middleware;
pub mod validation;

use crate::domain::{
    config::{ServerConfig, TokenRegistry},
    storage::StorageProvider,
};
use axum::{
    middleware::from_fn_with_state,
    routing::{get, put},
    Router,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState<T: StorageProvider> {
    pub storage: Arc<T>,
    pub config: Arc<ServerConfig>,
    pub token_registry: Arc<TokenRegistry>,
}

pub fn create_router<T: StorageProvider + Clone>(app_state: &AppState<T>) -> Router<AppState<T>> {
    let protected_routes = Router::new()
        .route("/v1/cache/{hash}", get(handlers::retrieve_artifact::<T>))
        .route("/v1/cache/{hash}", put(handlers::store_artifact::<T>))
        .route_layer(from_fn_with_state(
            app_state.clone(),
            middleware::auth_middleware::<T>,
        ));

    // Combine public and protected routes
    Router::new()
        .route("/health", get(handlers::health_check)) // Public route - no auth required
        .merge(protected_routes)
}

pub async fn run_server<T: StorageProvider + Clone>(
    storage: T,
    config: &ServerConfig,
) -> Result<(), std::io::Error> {
    let token_registry = TokenRegistry::from_strings(&config.service_access_token)
        .expect("Token validation should have passed during config validation");

    // Log all configured tokens on server start
    tracing::info!(
        "Server starting with {} configured token(s)",
        token_registry.token_names().count()
    );
    for name in token_registry.token_names() {
        tracing::info!("  - Token configured: {}", name);
    }

    let app_state = AppState {
        storage: Arc::new(storage),
        config: Arc::new(config.clone()),
        token_registry: Arc::new(token_registry),
    };

    let app = create_router::<T>(&app_state).with_state(app_state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;

    tracing::info!("Server running on port {}", config.port);
    axum::serve(listener, app).await?;

    Ok(())
}
