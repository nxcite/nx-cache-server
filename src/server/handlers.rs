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

    // Nx fails the task on any response to a PUT other than 200/409/403 (see
    // `store` below), so a failed HeadObject must not become a 500. Treat it as
    // absent and go on to store: keys are content-addressed, so re-writing
    // bytes that may already be there is a byte-identical no-op.
    if state.storage.exists(&hash).await.unwrap_or(false) {
        // Same reason as the 403 in auth_middleware: let the client finish
        // uploading, or it never sees this 409. Keys are content-addressed, so
        // the copy being discarded is byte-identical to the stored one.
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

    // A failed write is answered 403, never 500. Nx's `store()`
    // (packages/nx/src/native/cache/http_remote_cache.rs) returns Ok(true) on
    // 200 and Ok(false) on 409/403 - "not stored, carry on". Any other status
    // is "Misconfigured remote cache endpoint": `cache.put()` retries it six
    // times, re-uploading the artifact each time, then rejects, and the task
    // orchestrator marks the task - which already succeeded - as failed. The
    // only cost of the 403 is a later cache miss for this hash. The S3 error
    // is logged here; alert on that, not on the status code.
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

    // A read that fails is a cache miss. Nx handles 404 by running the task;
    // any other status fails it as a misconfigured endpoint, after every task
    // in the run already succeeded. Worst case here is recomputing an
    // artifact we already had.
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
