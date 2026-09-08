pub mod error;
pub mod handlers;
pub mod middleware;
pub mod validation;

use crate::domain::{
    config::ServerConfig,
    storage::{StorageProvider, PROBE_KEY},
};
use axum::{
    body::Body,
    middleware::from_fn_with_state,
    routing::{get, put},
    Router,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;

/// Whether the last periodic probe reached storage. `/health` reports it.
pub(crate) static STORAGE_REACHABLE: AtomicBool = AtomicBool::new(true);
const STORAGE_PROBE_INTERVAL: Duration = Duration::from_secs(60);

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

    // Credentials that expire while the server runs make every cache call
    // fail with nothing telling the operator. A HeadObject of the key the
    // startup probe wrote, once a minute, and /health carries the result so
    // the orchestrator can restart or alert. `interval` ticks immediately.
    let probe_storage = app_state.storage.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(STORAGE_PROBE_INTERVAL);
        loop {
            ticker.tick().await;
            let reachable = probe_storage.exists(PROBE_KEY).await.is_ok();
            if STORAGE_REACHABLE.swap(reachable, Ordering::Relaxed) != reachable {
                tracing::warn!(reachable, "storage reachability changed");
            }
        }
    });

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
    use axum::{
        body::to_bytes,
        http::{Request, StatusCode},
    };
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::{
        io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
        sync::RwLock,
    };
    use tokio_util::io::ReaderStream;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct AbsentStorage;

    #[derive(Clone)]
    struct PresentStorage;

    #[derive(Clone, Default)]
    struct MemoryStorage {
        entries: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    }

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

    #[async_trait::async_trait]
    impl StorageProvider for PresentStorage {
        async fn exists(&self, _hash: &str) -> Result<bool, StorageError> {
            Ok(true)
        }

        async fn store(
            &self,
            _hash: &str,
            _data: ReaderStream<impl AsyncRead + Send + Unpin>,
        ) -> Result<(), StorageError> {
            panic!("a collision must not reach storage")
        }

        async fn retrieve(
            &self,
            _hash: &str,
        ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
            Err(StorageError::NotFound)
        }
    }

    #[async_trait::async_trait]
    impl StorageProvider for MemoryStorage {
        async fn exists(&self, hash: &str) -> Result<bool, StorageError> {
            Ok(self.entries.read().await.contains_key(hash))
        }

        async fn store(
            &self,
            hash: &str,
            mut data: ReaderStream<impl AsyncRead + Send + Unpin>,
        ) -> Result<(), StorageError> {
            let mut bytes = Vec::new();
            while let Some(chunk) = data.next().await {
                bytes.extend_from_slice(&chunk.map_err(|_| StorageError::OperationFailed)?);
            }
            self.entries.write().await.insert(hash.to_owned(), bytes);
            Ok(())
        }

        async fn retrieve(
            &self,
            hash: &str,
        ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
            let bytes = self
                .entries
                .read()
                .await
                .get(hash)
                .cloned()
                .ok_or(StorageError::NotFound)?;
            Ok(Box::new(std::io::Cursor::new(bytes)))
        }
    }

    fn test_config() -> ServerConfig {
        ServerConfig {
            port: 0,
            bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            service_access_token: "read-write-token".to_string(),
            read_only_access_token: Some("read-only-token".to_string()),
            debug: false,
        }
    }

    fn authorized_request(method: &str, path: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", "Bearer read-write-token")
            .body(body)
            .unwrap()
    }

    fn test_app<T: StorageProvider + Clone>(storage: T) -> Router {
        let app_state = AppState {
            storage: Arc::new(storage),
            config: Arc::new(test_config()),
        };
        create_router(&app_state).with_state(app_state)
    }

    #[tokio::test]
    async fn successful_upload_returns_ok_and_preserves_artifact_bytes() {
        let storage = MemoryStorage::default();
        let app = test_app(storage.clone());
        let artifact = b"exact artifact bytes\0\xff";

        let response = app
            .oneshot(authorized_request(
                "PUT",
                "/v1/cache/deadbeef",
                Body::from(artifact.as_slice()),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(storage.entries.read().await["deadbeef"], artifact);
    }

    #[tokio::test]
    async fn retrieve_returns_exact_artifact_with_binary_content_type() {
        let storage = MemoryStorage::default();
        let artifact = b"exact artifact bytes\0\xff";
        storage
            .entries
            .write()
            .await
            .insert("deadbeef".to_owned(), artifact.to_vec());
        let app = test_app(storage);

        let response = app
            .oneshot(authorized_request(
                "GET",
                "/v1/cache/deadbeef",
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["content-type"],
            "application/octet-stream"
        );
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            artifact.as_slice()
        );
    }

    #[tokio::test]
    async fn collision_does_not_replace_the_stored_artifact() {
        let storage = MemoryStorage::default();
        let artifact = b"original artifact";
        storage
            .entries
            .write()
            .await
            .insert("deadbeef".to_owned(), artifact.to_vec());
        let app = test_app(storage.clone());

        let response = app
            .oneshot(authorized_request(
                "PUT",
                "/v1/cache/deadbeef",
                Body::from("replacement"),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(storage.entries.read().await["deadbeef"], artifact);
    }

    #[tokio::test]
    async fn health_check_is_public_and_reports_storage() {
        let health = || {
            test_app(MemoryStorage::default())
                .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        };

        let response = health().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "OK"
        );

        STORAGE_REACHABLE.store(false, Ordering::Relaxed);
        let response = health().await.unwrap();
        STORAGE_REACHABLE.store(true, Ordering::Relaxed);
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn collision_is_reported_without_closing_the_upload() {
        let app_state = AppState {
            storage: Arc::new(PresentStorage),
            config: Arc::new(test_config()),
        };
        let app = create_router::<PresentStorage>(&app_state).with_state(app_state);

        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        const BODY_LEN: usize = 8 * 1024 * 1024;
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                format!(
                    "PUT /v1/cache/deadbeef HTTP/1.1\r\nHost: localhost\r\n\
                     Authorization: Bearer read-write-token\r\nContent-Length: {BODY_LEN}\r\n\r\n"
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
            status_line.starts_with("HTTP/1.1 409"),
            "expected a 409 status line, got: {status_line}"
        );
    }

    /// A refused write must still reach the client as a 403. The client is
    /// mid-upload when the decision is made, so the body has to be taken to
    /// completion first — otherwise the connection closes under it and the
    /// client only ever sees a write error.
    #[tokio::test]
    async fn read_only_write_is_refused_without_closing_the_upload() {
        let app_state = AppState {
            storage: Arc::new(AbsentStorage),
            config: Arc::new(test_config()),
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
