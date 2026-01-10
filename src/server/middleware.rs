use crate::domain::storage::StorageProvider;
use crate::server::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;
use tracing;

pub async fn auth_middleware<T>(
    State(state): State<AppState<T>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>
where
    T: StorageProvider,
{
    // Extract Bearer token from Authorization header
    let token = request
        .headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|auth_value| auth_value.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Check token against all configured tokens using constant-time comparison
    let mut matched_name: Option<&str> = None;

    for token_value in state.token_registry.tokens() {
        if bool::from(token.as_bytes().ct_eq(token_value.as_bytes())) {
            matched_name = state.token_registry.find_token_name(token_value);
            break;
        }
    }

    match matched_name {
        Some(name) => {
            tracing::info!("Authenticated request from: {}", name);
            Ok(next.run(request).await)
        }
        None => {
            tracing::warn!("Authentication failed: invalid token");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
