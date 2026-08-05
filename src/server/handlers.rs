use crate::domain::storage::StorageProvider;
use crate::server::{error::ServerError, validation, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use tokio_stream::StreamExt;

pub async fn store_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;

    if state.storage.exists(&hash).await? {
        return Ok((StatusCode::CONFLICT, "Cannot override an existing record"));
    }

    let content_length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    match content_length {
        // Stream the body straight through to storage without buffering.
        Some(length) => {
            let stream = body
                .into_data_stream()
                .map(|chunk| chunk.map_err(std::io::Error::other));
            state.storage.store(&hash, stream, length).await?;
        }
        // No Content-Length (e.g. chunked transfer encoding): buffer to learn
        // the size, since object stores require it up front.
        None => {
            let bytes = axum::body::to_bytes(body, usize::MAX)
                .await
                .map_err(|_| ServerError::BadRequest)?;
            let length = bytes.len() as u64;
            let stream = tokio_stream::once(Ok(bytes));
            state.storage.store(&hash, stream, length).await?;
        }
    }

    Ok((StatusCode::ACCEPTED, ""))
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
