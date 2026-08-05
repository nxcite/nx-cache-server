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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::storage::StorageError;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
    use tokio_util::io::ReaderStream;

    #[derive(Clone)]
    struct AbsentStorage;

    #[async_trait::async_trait]
    impl StorageProvider for AbsentStorage {
        async fn exists(&self, _hash: &str) -> Result<bool, StorageError> {
            Ok(false)
        }

        async fn store(
            &self,
            _hash: &str,
            _data: ReaderStream<impl AsyncRead + Send + Unpin>,
        ) -> Result<(), StorageError> {
            Ok(())
        }

        async fn retrieve(
            &self,
            _hash: &str,
        ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
            Err(StorageError::NotFound)
        }
    }

    /// A refused write must still reach the client as a 403. The client is
    /// mid-upload when the decision is made, so the body has to be taken to
    /// completion first — otherwise the connection closes under it and the
    /// client only ever sees a write error.
    #[tokio::test]
    async fn read_only_write_is_refused_without_closing_the_upload() {
        let app_state = AppState {
            storage: Arc::new(AbsentStorage),
            config: Arc::new(ServerConfig {
                port: 0,
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                service_access_token: "read-write-token".to_string(),
                read_only_access_token: Some("read-only-token".to_string()),
                debug: false,
            }),
        };
        let app = create_router::<AbsentStorage>(&app_state).with_state(app_state);

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        // Bigger than the socket buffers, so the response is necessarily
        // decided while the upload is still in flight. A body small enough to
        // fit in the kernel buffer passes with or without the drain.
        const BODY_LEN: usize = 8 * 1024 * 1024;
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                format!(
                    "PUT /v1/cache/deadbeef HTTP/1.1\r\nHost: localhost\r\n\
                     Authorization: Bearer read-only-token\r\nContent-Length: {BODY_LEN}\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();

        let chunk = vec![0u8; 64 * 1024];
        let mut sent = 0;
        while sent < BODY_LEN {
            stream
                .write_all(&chunk)
                .await
                .expect("connection closed while the client was still uploading");
            sent += chunk.len();
        }

        let mut status_line = String::new();
        BufReader::new(stream)
            .read_line(&mut status_line)
            .await
            .unwrap();
        assert!(
            status_line.starts_with("HTTP/1.1 403"),
            "expected a 403 status line, got: {status_line}"
        );
    }
}
