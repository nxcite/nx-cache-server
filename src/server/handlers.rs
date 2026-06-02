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

    if state.storage.exists(&hash).await? {
        // The Nx client only ever sends the opaque task hash, never the command
        // that produced it - the hash is the only identifier we can log here.
        tracing::info!("cache STORE skipped (already cached): {hash}");
        return Ok((StatusCode::CONFLICT, "Cannot override an existing record"));
    }

    // For now, let's use a simpler approach - collect the body into bytes
    // TODO: Implement true streaming later for better memory efficiency
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|_| ServerError::BadRequest)?;

    let size = bytes.len();
    let cursor = std::io::Cursor::new(bytes);
    let reader_stream = tokio_util::io::ReaderStream::new(cursor);

    state.storage.store(&hash, reader_stream).await?;

    tracing::info!("cache STORE: {hash} ({})", human_size(size));

    Ok((StatusCode::ACCEPTED, ""))
}

pub async fn retrieve_artifact<T: StorageProvider>(
    Path(hash): Path<String>,
    State(state): State<AppState<T>>,
) -> Result<impl IntoResponse, ServerError> {
    validation::validate_hash(&hash)?;

    // The hash is the only identifier the Nx client sends - it never includes
    // the command/target that produced the artifact, so that is all we can log.
    let reader = match state.storage.retrieve(&hash).await {
        Ok(reader) => {
            tracing::info!("cache HIT: {hash}");
            reader
        }
        Err(StorageError::NotFound) => {
            tracing::info!("cache MISS: {hash}");
            return Err(StorageError::NotFound.into());
        }
        Err(e) => {
            return Err(e.into());
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

/// Format a byte count as a short, human-readable string (e.g. `24.5 KB`).
fn human_size(bytes: usize) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
