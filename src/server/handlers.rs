//! Status codes toward Nx follow its HTTP remote cache client
//! (`packages/nx/src/native/cache/http_remote_cache.rs`), which cannot be
//! deduced from this repo:
//!
//! - PUT: 200 = stored, 409/403 = "not stored, carry on". Anything else is
//!   "Misconfigured remote cache endpoint": `cache.put()` retries six times,
//!   re-uploading each time, then the task that already succeeded is marked
//!   failed.
//! - GET: 200 = hit, 404 = miss (run the task). Anything else fails the task.
//!
//! So a storage failure is answered 403 on a write and 404 on a read, never
//! 5xx. The S3 error is logged at ERROR at the point of failure.

use crate::domain::storage::{StorageError, StorageProvider};
use crate::server::{error::ServerError, validation, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

pub async fn store_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
    body: Body,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;

    // A failed HeadObject is treated as absent: keys are content-addressed, so
    // re-writing bytes that may already be there is a byte-identical no-op.
    if state.storage.exists(&hash).await.unwrap_or(false) {
        // Same reason as the 403 in auth_middleware: let the client finish
        // uploading, or it never sees this 409.
        crate::server::drain_body(body).await;
        return Ok((StatusCode::CONFLICT, "Cannot override an existing record"));
    }

    // For now, let's use a simpler approach - collect the body into bytes
    // TODO: Implement true streaming later for better memory efficiency
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|_| ServerError::BadRequest)?;

    let cursor = std::io::Cursor::new(bytes);
    let reader_stream = tokio_util::io::ReaderStream::new(cursor);

    if let Err(e) = state.storage.store(&hash, reader_stream).await {
        tracing::error!(
            hash,
            "storing artifact failed: {e}; answering 403 so Nx does not fail the task"
        );
        return Err(ServerError::Forbidden);
    }

    Ok((StatusCode::ACCEPTED, ""))
}

pub async fn retrieve_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;

    let reader = match state.storage.retrieve(&hash).await {
        Ok(reader) => reader,
        Err(StorageError::NotFound) => return Err(StorageError::NotFound.into()),
        Err(e) => {
            tracing::error!(
                hash,
                "retrieving artifact failed: {e}; answering 404 so Nx runs the task"
            );
            return Err(StorageError::NotFound.into());
        }
    };
    let stream = tokio_util::io::ReaderStream::new(reader);
    let body = Body::from_stream(stream);

    Ok((
        StatusCode::OK,
        [("content-type", "application/octet-stream")],
        body,
    ))
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
