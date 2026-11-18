use crate::domain::storage::StorageError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Bad request")]
    BadRequest,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Internal server error")]
    InternalError,

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            // Map domain errors to HTTP responses
            ServerError::Storage(StorageError::NotFound) => {
                (StatusCode::NOT_FOUND, "The record was not found")
            }
            ServerError::Storage(StorageError::AlreadyExists) => {
                (StatusCode::CONFLICT, "Cannot override an existing record")
            }
            ServerError::Storage(StorageError::ClientAbort) => {
                (StatusCode::BAD_REQUEST, "Bad request")
            }

            // HTTP-specific errors
            ServerError::BadRequest => (StatusCode::BAD_REQUEST, "Bad request"),
            ServerError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            ServerError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),

            // Generic fallback - log details but return safe message
            _ => {
                tracing::error!("Server error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        // Exactly "text/plain", no "; charset=utf-8": on a 401 Nx compares the
        // header byte for byte and otherwise reports "Requests should respond
        // with text/plain on 401s" instead of the auth error
        // (nrwl/nx packages/nx/src/native/cache/errors.rs, convert_response_to_error).
        (status, [("Content-Type", "text/plain")], message).into_response()
    }
}
