use crate::domain::storage::StorageProvider;
use crate::server::{error::ServerError, AppState};
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use subtle::ConstantTimeEq;

pub async fn auth_middleware<T, B>(
    State(state): State<AppState<T>>,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, ServerError>
where
    T: StorageProvider,
{
    // Extract Bearer token from Authorization header
    let token = request
        .headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|auth_value| auth_value.strip_prefix("Bearer "))
        .ok_or(ServerError::Unauthorized)?;

    // Constant-time comparison for security
    if !token
        .as_bytes()
        .ct_eq(state.config.service_access_token.as_bytes())
        .into()
    {
        return Err(ServerError::Unauthorized);
    }

    Ok(next.run(request).await)
}
