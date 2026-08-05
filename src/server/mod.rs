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
use std::time::Duration;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct AppState<T: StorageProvider> {
    pub storage: Arc<T>,
    pub config: Arc<ServerConfig>,
}

/// How long we go on reading a body we already know we are discarding. Draining
/// is a courtesy to the client and must not become a way for one to hold a
/// connection open: there is no request timeout and no body size limit anywhere
/// else in the stack, and the read-only token is by design the one handed to
/// callers we don't trust. Exceeding this restores the old behaviour for that one
/// client (it sees a write error) rather than tying up the server indefinitely.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(60);

/// Read and discard a request body so the client can finish uploading before a
/// response ends the exchange. An unread body forces the connection shut, which
/// reaches the client as a write error instead of the status it was sent. A
/// read error means the client is already gone — nothing left to drain.
pub(crate) async fn drain_body(body: Body) {
    drain_body_within(body, DRAIN_TIMEOUT).await;
}

/// Split out so a test can bound it in milliseconds instead of waiting a minute.
async fn drain_body_within(body: Body, limit: Duration) {
    let _ = tokio::time::timeout(limit, async move {
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
    use axum::http::{header, Request, StatusCode};
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
    use tokio_util::io::ReaderStream;
    use tower::ServiceExt;

    #[derive(Clone, Copy, Debug)]
    enum ExistsBehavior {
        No,
        Yes,
        Fail,
    }

    #[derive(Clone)]
    struct MockStorage {
        exists: ExistsBehavior,
        store_fails: bool,
    }

    impl MockStorage {
        fn absent() -> Self {
            Self {
                exists: ExistsBehavior::No,
                store_fails: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl StorageProvider for MockStorage {
        async fn exists(&self, _hash: &str) -> Result<bool, StorageError> {
            match self.exists {
                ExistsBehavior::No => Ok(false),
                ExistsBehavior::Yes => Ok(true),
                ExistsBehavior::Fail => Err(StorageError::OperationFailed),
            }
        }

        async fn store(
            &self,
            _hash: &str,
            _data: ReaderStream<impl AsyncRead + Send + Unpin>,
        ) -> Result<(), StorageError> {
            if self.store_fails {
                return Err(StorageError::OperationFailed);
            }
            Ok(())
        }

        async fn retrieve(
            &self,
            _hash: &str,
        ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
            match self.exists {
                ExistsBehavior::Yes => Ok(Box::new(std::io::Cursor::new(b"artifact".to_vec()))),
                _ => Err(StorageError::NotFound),
            }
        }
    }

    const RW_TOKEN: &str = "read-write-token";
    const RO_TOKEN: &str = "read-only-token";
    const VALID_HASH: &str = "deadbeef";
    const INVALID_HASH: &str = "bad.hash!";

    fn app(storage: MockStorage) -> Router {
        let app_state = AppState {
            storage: Arc::new(storage),
            config: Arc::new(ServerConfig {
                port: 3000,
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                service_access_token: RW_TOKEN.to_string(),
                read_only_access_token: Some(RO_TOKEN.to_string()),
                debug: false,
            }),
        };
        create_router::<MockStorage>(&app_state).with_state(app_state)
    }

    /// A request body whose consumption is observable: the counter advances only
    /// when a chunk is actually polled out of it. A response sent while chunks
    /// remain means the body was dropped unread, which over a real socket is the
    /// client's broken pipe. This is why `oneshot` alone cannot see this class of
    /// bug - it silently discards whatever the handler didn't read.
    fn counted_body(chunks: usize) -> (Body, Arc<AtomicUsize>) {
        let read = Arc::new(AtomicUsize::new(0));
        let counter = read.clone();
        let stream = tokio_stream::iter(0..chunks).map(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok::<Vec<u8>, std::io::Error>(vec![0u8; 1024])
        });
        (Body::from_stream(stream), read)
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum Token {
        Rw,
        Ro,
        Wrong,
        None,
    }

    /// Must the request body be read to completion before we answer? An exception
    /// has to name whose upload it hangs up on and why that is acceptable, so the
    /// decision shows up in review rather than being silently forgotten.
    #[derive(Clone, Copy)]
    enum Drain {
        Required,
        NotRequired(&'static str),
    }

    struct Case {
        method: &'static str,
        hash: &'static str,
        token: Token,
        exists: ExistsBehavior,
        store_fails: bool,
        status: StatusCode,
        drain: Drain,
    }

    /// 32 KiB in 1 KiB chunks. Size is irrelevant in-process (no socket buffers
    /// are involved); more than one chunk is all it takes to tell "read it all"
    /// from "dropped it".
    const BODY_CHUNKS: usize = 32;

    fn all_cases() -> Vec<Case> {
        let case = |method, token, exists, status, drain| Case {
            method,
            hash: VALID_HASH,
            token,
            exists,
            store_fails: false,
            status,
            drain,
        };
        let unauthenticated = "an unauthenticated caller is owed nothing, and reading a body \
             before deciding whether the caller may speak at all would let anyone make \
             us read";
        use ExistsBehavior::{No, Yes};
        vec![
            // --- the method x token cross-product (see cross_product_is_covered) ---
            case("GET", Token::Rw, Yes, StatusCode::OK, Drain::Required),
            case("GET", Token::Ro, Yes, StatusCode::OK, Drain::Required),
            case(
                "GET",
                Token::Wrong,
                Yes,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            case(
                "GET",
                Token::None,
                Yes,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            // HEAD is a read, and axum routes it to the GET handler, so the
            // read-only token must not be refused it.
            case("HEAD", Token::Rw, Yes, StatusCode::OK, Drain::Required),
            case("HEAD", Token::Ro, Yes, StatusCode::OK, Drain::Required),
            case(
                "HEAD",
                Token::Wrong,
                Yes,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            case(
                "HEAD",
                Token::None,
                Yes,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            // The CREEP refusal: the client is mid-upload when we decide, so the
            // 403 only reaches it if the upload is taken to completion first.
            case("PUT", Token::Ro, No, StatusCode::FORBIDDEN, Drain::Required),
            case("PUT", Token::Rw, No, StatusCode::ACCEPTED, Drain::Required),
            case(
                "PUT",
                Token::Wrong,
                No,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            case(
                "PUT",
                Token::None,
                No,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            // `route_layer` does run on a method mismatch, so auth answers first
            // and a read-only POST is a drained 403 rather than a 405.
            case(
                "POST",
                Token::Rw,
                No,
                StatusCode::METHOD_NOT_ALLOWED,
                Drain::NotRequired(
                    "axum's built-in 405, whose body we never see. Reaching it needs the \
                     write token and a method no Nx client sends, so a \
                     `method_not_allowed_fallback` handler would buy nothing",
                ),
            ),
            case(
                "POST",
                Token::Ro,
                No,
                StatusCode::FORBIDDEN,
                Drain::Required,
            ),
            case(
                "POST",
                Token::Wrong,
                No,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            case(
                "POST",
                Token::None,
                No,
                StatusCode::UNAUTHORIZED,
                Drain::NotRequired(unauthenticated),
            ),
            // --- the remaining response classes a body-carrying PUT can reach ---
            // Write-once refusal, same mid-upload timing as the 403.
            case("PUT", Token::Rw, Yes, StatusCode::CONFLICT, Drain::Required),
            // A failed existence probe answers 500 before the body is touched.
            case(
                "PUT",
                Token::Rw,
                ExistsBehavior::Fail,
                StatusCode::INTERNAL_SERVER_ERROR,
                Drain::Required,
            ),
            // The key is rejected before the body is looked at.
            Case {
                method: "PUT",
                hash: INVALID_HASH,
                token: Token::Rw,
                exists: No,
                store_fails: false,
                status: StatusCode::BAD_REQUEST,
                drain: Drain::Required,
            },
            // Auth precedes validation, so a read-only token sees 403, not 400.
            Case {
                method: "PUT",
                hash: INVALID_HASH,
                token: Token::Ro,
                exists: No,
                store_fails: false,
                status: StatusCode::FORBIDDEN,
                drain: Drain::Required,
            },
            // A failed write: the handler buffers the body before storing, so it
            // is already consumed by the time this is answered.
            Case {
                method: "PUT",
                hash: VALID_HASH,
                token: Token::Rw,
                exists: No,
                store_fails: true,
                status: StatusCode::INTERNAL_SERVER_ERROR,
                drain: Drain::Required,
            },
            // GET/HEAD carry no body, so draining is trivially satisfied; these
            // rows are here for the status assertions.
            case("GET", Token::Ro, No, StatusCode::NOT_FOUND, Drain::Required),
            case(
                "HEAD",
                Token::Ro,
                No,
                StatusCode::NOT_FOUND,
                Drain::Required,
            ),
        ]
    }

    /// Deciding a response without reading the request body closes the connection
    /// under a client that is still uploading, so it never sees the status. This is
    /// the whole bug class in one table: every response the cache route can produce
    /// for a body-carrying request must consume that body first, or carry a
    /// written-down reason for not doing so.
    #[tokio::test]
    async fn every_response_drains_the_body() {
        let mut failures = Vec::new();

        for case in all_cases() {
            let storage = MockStorage {
                exists: case.exists,
                store_fails: case.store_fails,
            };
            let carries_body = matches!(case.method, "PUT" | "POST");
            let (body, read) = if carries_body {
                counted_body(BODY_CHUNKS)
            } else {
                (Body::empty(), Arc::new(AtomicUsize::new(0)))
            };

            let mut request = Request::builder()
                .method(case.method)
                .uri(format!("/v1/cache/{}", case.hash));
            match case.token {
                Token::Rw => {
                    request = request.header(header::AUTHORIZATION, format!("Bearer {RW_TOKEN}"))
                }
                Token::Ro => {
                    request = request.header(header::AUTHORIZATION, format!("Bearer {RO_TOKEN}"))
                }
                Token::Wrong => {
                    request = request.header(header::AUTHORIZATION, "Bearer wrong-token")
                }
                Token::None => {}
            }

            let label = format!(
                "{} /v1/cache/{} as {:?} (exists={:?}, store_fails={})",
                case.method, case.hash, case.token, case.exists, case.store_fails
            );
            let response = app(storage)
                .oneshot(request.body(body).unwrap())
                .await
                .unwrap();

            if response.status() != case.status {
                failures.push(format!(
                    "{label}: expected {}, got {}",
                    case.status,
                    response.status()
                ));
                continue;
            }

            if carries_body {
                let chunks_read = read.load(Ordering::SeqCst);
                match case.drain {
                    Drain::Required if chunks_read < BODY_CHUNKS => failures.push(format!(
                        "{label}: answered {} after reading only {chunks_read}/{BODY_CHUNKS} \
                         body chunks - the client would see a write error, not the status",
                        case.status
                    )),
                    // Hanging up on a mid-upload client has to be a decision
                    // someone wrote down, not an oversight that reads the same.
                    Drain::NotRequired(reason) => assert!(
                        !reason.is_empty(),
                        "{label}: answering without draining needs a stated reason"
                    ),
                    Drain::Required => {}
                }
            }
        }

        assert!(
            failures.is_empty(),
            "{} case(s) failed:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }

    /// The table above is only as good as its coverage. A new method or a new
    /// token class cannot be added without a row deciding what it answers and
    /// whether it drains.
    #[tokio::test]
    async fn cross_product_is_covered() {
        let covered: std::collections::HashSet<_> = all_cases()
            .iter()
            .map(|case| (case.method, case.token))
            .collect();
        let mut missing = Vec::new();
        for method in ["GET", "HEAD", "PUT", "POST"] {
            for token in [Token::Rw, Token::Ro, Token::Wrong, Token::None] {
                if !covered.contains(&(method, token)) {
                    missing.push(format!("{method} as {token:?}"));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "uncovered method/token pairs: {missing:?}"
        );
    }

    /// What makes the hardcoded method list above a guard rather than a wish:
    /// every method the table does *not* cover has to be a 405. Adding a method
    /// to the router without adding rows for it turns its 405 into something
    /// else, and fails here.
    #[tokio::test]
    async fn methods_outside_the_table_are_rejected() {
        let covered: std::collections::HashSet<_> =
            all_cases().iter().map(|case| case.method).collect();
        for method in ["DELETE", "PATCH", "OPTIONS", "TRACE"] {
            if covered.contains(&method) {
                continue;
            }
            let response = app(MockStorage {
                exists: ExistsBehavior::Yes,
                store_fails: false,
            })
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/v1/cache/{VALID_HASH}"))
                    // The write token, so this is a routing answer and not an
                    // auth one.
                    .header(header::AUTHORIZATION, format!("Bearer {RW_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} is routed somewhere but has no rows in all_cases(), so nothing \
                 decides what it answers or whether it drains"
            );
        }
    }

    /// An unbounded drain would let one authenticated client hold a connection
    /// open indefinitely by never finishing its upload.
    #[tokio::test]
    async fn drain_gives_up_on_a_body_that_never_ends() {
        let never_ends =
            Body::from_stream(tokio_stream::pending::<Result<Vec<u8>, std::io::Error>>());
        // Returns, rather than hanging the suite.
        drain_body_within(never_ends, Duration::from_millis(50)).await;
    }

    /// A refused write must still reach the client as a 403. The client is
    /// mid-upload when the decision is made, so the body has to be taken to
    /// completion first — otherwise the connection closes under it and the
    /// client only ever sees a write error.
    #[tokio::test]
    async fn read_only_write_is_refused_without_closing_the_upload() {
        let app_state = AppState {
            storage: Arc::new(MockStorage::absent()),
            config: Arc::new(ServerConfig {
                port: 0,
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                service_access_token: "read-write-token".to_string(),
                read_only_access_token: Some("read-only-token".to_string()),
                debug: false,
            }),
        };
        let app = create_router::<MockStorage>(&app_state).with_state(app_state);

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
