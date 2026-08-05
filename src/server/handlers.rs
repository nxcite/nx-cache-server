use crate::domain::storage::StorageProvider;
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
    if let Err(invalid) = validation::validate_hash(&hash) {
        // Same reason as the 409 below: the key is rejected before the body has
        // been touched, and answering with it unread closes the connection under
        // a client that is still uploading, so it sees a write error not a 400.
        crate::server::drain_body(body).await;
        return Err(invalid);
    }

    let exists = match state.storage.exists(&hash).await {
        Ok(exists) => exists,
        Err(err) => {
            // And again: a failed HeadObject answers 500 without the body having
            // been read.
            crate::server::drain_body(body).await;
            return Err(err.into());
        }
    };

    if exists {
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
