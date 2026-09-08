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
    // without being able to poison it (CVE-2025-36852 / CREEP). HEAD is a read
    // and axum routes it to the GET handler, so it belongs on this side of the
    // line. The check stays an allowlist, so a method added later fails closed.
    let method = request.method();
    let is_read = method == Method::GET || method == Method::HEAD;
    if !is_read_write && !is_read {
        // Take the upload to completion before answering. Responding while the
        // client is still sending leaves an unread request body, so the
        // connection is closed under it: the client sees a write error rather
        // than this 403, and Nx fails the task even though it treats a 403
        // itself as "not stored, carry on". Only authenticated callers get
        // here, so no untrusted body is read.
        crate::server::drain_body(request.into_body()).await;
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}
