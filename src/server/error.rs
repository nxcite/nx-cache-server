use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;
use crate::domain::storage::StorageError;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Bad request")]
    BadRequest,
    
    #[error("Unauthorized")]
    Unauthorized,
    
    #[error("Internal server error")]
    InternalError,
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            // Map domain errors to HTTP responses
            ServerError::Storage(StorageError::NotFound) => 
                (StatusCode::NOT_FOUND, "The record was not found"),
            ServerError::Storage(StorageError::AlreadyExists) => 
                (StatusCode::CONFLICT, "Cannot override an existing record"),
            
            // HTTP-specific errors
            ServerError::BadRequest => 
                (StatusCode::BAD_REQUEST, "Bad request"),
            ServerError::Unauthorized => 
                (StatusCode::UNAUTHORIZED, "Unauthorized"),
            
            // Generic fallback - log details but return safe message
            _ => {
                tracing::error!("Server error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };
        
        (status, [("Content-Type", "text/plain")], message).into_response()
    }
}