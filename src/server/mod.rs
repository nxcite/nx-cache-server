pub mod error;
pub mod handlers;
pub mod logging;
pub mod middleware;
pub mod validation;

use crate::domain::{config::ServerConfig, storage::StorageProvider};
use axum::{
    body::Body,
    middleware::{from_fn, from_fn_with_state},
    routing::{get, put},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct AppState<T: StorageProvider> {
    pub storage: Arc<T>,
    pub config: Arc<ServerConfig>,
}

/// How long we go on reading a body we already know we are discarding. There is
/// no request timeout and no body size limit anywhere else in the stack, so an
/// unbounded drain would let a caller hold a connection open by trickling a
/// large declared Content-Length. Past this, that one client gets the write
/// error it would have got without the drain.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(60);

/// Read and discard a request body so the client can finish uploading before a
/// response ends the exchange. An unread body forces the connection shut, which
/// reaches the client as a write error instead of the status it was sent. A
/// read error means the client is already gone — nothing left to drain.
pub(crate) async fn drain_body(body: Body) {
    let _ = tokio::time::timeout(DRAIN_TIMEOUT, async move {
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            if chunk.is_err() {
                break;
            }
        }
    })
    .await;
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
        // Outermost, so 401s and axum rejections are logged too.
        .layer(from_fn(logging::request_log))
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

    tracing::info!(%addr, "Server running on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
