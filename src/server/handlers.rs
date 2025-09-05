use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    body::Body,
};
use crate::domain::storage::StorageProvider;
use crate::server::{AppState, error::ServerError};

pub async fn store_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(_state): State<AppState<T>>,
    _headers: HeaderMap,
    _body: Body,
) -> Result<impl IntoResponse, ServerError> {
    // TODO: Implement store logic
    Ok((StatusCode::ACCEPTED, ""))
}

pub async fn retrieve_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(_state): State<AppState<T>>,
) -> Result<impl IntoResponse, ServerError> {
    // TODO: Implement retrieve logic
    Ok((StatusCode::OK, [("content-type", "application/octet-stream")], ""))
}