use crate::domain::storage::{StorageError, StorageProvider};
use crate::server::{
    error::ServerError,
    logging::{Outcome, RequestRecord},
    validation, AppState,
};
use axum::{
    body::Body,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
};

pub async fn store_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
    Extension(record): Extension<RequestRecord>,
    body: Body,
) -> Result<impl IntoResponse, ServerError> {
    // Every refusal before the body is read drains it first, for the reason given
    // on the 403 in auth_middleware: an unread body reaches an uploading client as
    // a write error instead of the status.
    if let Err(invalid) = validation::validate_hash(&hash) {
        crate::server::drain_body(body).await;
        return Err(invalid);
    }

    let exists = match state.storage.exists(&hash).await {
        Ok(exists) => exists,
        Err(err) => {
            crate::server::drain_body(body).await;
            return Err(err.into());
        }
    };

    if exists {
        // Keys are content-addressed, so the copy being discarded is
        // byte-identical to the stored one.
        crate::server::drain_body(body).await;
        return Ok((StatusCode::CONFLICT, "Cannot override an existing record"));
    }

    // For now, let's use a simpler approach - collect the body into bytes
    // TODO: Implement true streaming later for better memory efficiency
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|_| ServerError::BadRequest)?;

    let length = bytes.len() as u64;
    let cursor = std::io::Cursor::new(bytes);
    let reader_stream = tokio_util::io::ReaderStream::new(cursor);

    state.storage.store(&hash, reader_stream).await?;
    record.set_outcome(Outcome::Stored);
    record.set_bytes(length);

    Ok((StatusCode::OK, ""))
}

pub async fn retrieve_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
    Extension(record): Extension<RequestRecord>,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;

    let retrieved = state.storage.retrieve(&hash).await.inspect_err(|e| {
        if matches!(e, StorageError::NotFound) {
            record.set_outcome(Outcome::Miss);
        }
    })?;
    record.set_outcome(Outcome::Hit);
    record.set_bytes(retrieved.content_length.unwrap_or(0));
    let stream = tokio_util::io::ReaderStream::new(retrieved.reader);
    let body = Body::from_stream(stream);

    Ok((
        StatusCode::OK,
        [("content-type", "application/octet-stream")],
        body,
    ))
}

pub async fn health_check(Extension(record): Extension<RequestRecord>) -> impl IntoResponse {
    record.set_outcome(Outcome::Health);
    (StatusCode::OK, "OK")
}
