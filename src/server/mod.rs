pub mod error;
pub mod handlers;
pub mod middleware;
pub mod validation;

use crate::domain::{config::ServerConfig, storage::StorageProvider};
use axum::{
    body::Body,
    middleware::from_fn_with_state,
    routing::{get, put},
    Router,
};
use std::sync::Arc;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct AppState<T: StorageProvider> {
    pub storage: Arc<T>,
    pub config: Arc<ServerConfig>,
}

/// Read and discard a request body so the client can finish uploading before a
/// response ends the exchange. An unread body forces the connection shut, which
/// reaches the client as a write error instead of the status it was sent. A
/// read error means the client is already gone — nothing left to drain.
pub(crate) async fn drain_body(body: Body) {
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        if chunk.is_err() {
            break;
        }
    }
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
    let app_state = AppState {
        storage: Arc::new(storage),
        config: Arc::new(config.clone()),
    };

    let app = create_router::<T>(&app_state).with_state(app_state);
    let addr = std::net::SocketAddr::new(config.bind_address, config.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Server running on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
