use crate::domain::storage::StorageProvider;
use crate::server::{error::ServerError, validation, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use tokio_stream::StreamExt;

pub async fn store_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
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

    let body_stream = body
        .into_data_stream()
        .map(|chunk| chunk.map_err(std::io::Error::other));
    let mut reader = tokio_util::io::StreamReader::new(body_stream);

    // Lent, not moved: if storage fails part-way, the rest of the upload is
    // still here to drain.
    let stored = state
        .storage
        .store(&hash, tokio_util::io::ReaderStream::new(&mut reader))
        .await;
    if stored.is_err() {
        let _ = tokio::time::timeout(
            crate::server::DRAIN_TIMEOUT,
            tokio::io::copy(&mut reader, &mut tokio::io::sink()),
        )
        .await;
    }
    stored?;

    Ok((StatusCode::OK, ""))
}

pub async fn retrieve_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;

    let reader = state.storage.retrieve(&hash).await?;
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
