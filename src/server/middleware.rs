use crate::domain::storage::StorageProvider;
use crate::server::AppState;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
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

    // Constant-time comparisons for security. Both tokens are always
    // compared so timing does not reveal which one matched.
    let is_read_write = bool::from(
        token
            .as_bytes()
            .ct_eq(state.config.service_access_token.as_bytes()),
    );
    let is_read_only = state
        .config
        .read_only_access_token
        .as_deref()
        .is_some_and(|read_only| bool::from(token.as_bytes().ct_eq(read_only.as_bytes())));

    if !is_read_write && !is_read_only {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // The read-only token may only read; writes require the service access
    // token. This lets untrusted CI jobs (e.g. PR builds) use the cache
    // without being able to poison it (CVE-2025-36852 / CREEP).
    if !is_read_write && request.method() != Method::GET {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}
