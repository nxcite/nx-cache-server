use crate::domain::storage::StorageProvider;
use crate::server::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;

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

    // Constant-time comparison for security
    if !bool::from(
        token
            .as_bytes()
            .ct_eq(state.config.service_access_token.as_bytes()),
    ) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
