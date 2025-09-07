use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    body::Body,
};
use crate::domain::storage::StorageProvider;
use crate::server::{AppState, error::ServerError, validation};

pub async fn store_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
    body: Body,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;
    
    if state.storage.exists(&hash).await? {
        return Ok((StatusCode::CONFLICT, "Cannot override an existing record"));
    }
    
    // For now, let's use a simpler approach - collect the body into bytes
    // TODO: Implement true streaming later for better memory efficiency
    let bytes = axum::body::to_bytes(body, usize::MAX).await
        .map_err(|_| ServerError::BadRequest)?;
    
    let cursor = std::io::Cursor::new(bytes);
    let reader_stream = tokio_util::io::ReaderStream::new(cursor);
    
    state.storage.store(&hash, reader_stream).await?;
    
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