use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use tracing::{Instrument, Level, Span};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    /// No valid token.
    #[default]
    None,
    ReadWrite,
    ReadOnly,
}

impl Access {
    fn as_str(self) -> &'static str {
        match self {
            Access::None => "none",
            Access::ReadWrite => "read_write",
            Access::ReadOnly => "read_only",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Hit,
    Miss,
    Stored,
    Exists,
    Forbidden,
    Unauthorized,
    Invalid,
    Error,
    Health,
    Aborted,
    Unmatched,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Hit => "hit",
            Outcome::Miss => "miss",
            Outcome::Stored => "stored",
            Outcome::Exists => "exists",
            Outcome::Forbidden => "forbidden",
            Outcome::Unauthorized => "unauthorized",
            Outcome::Invalid => "invalid",
            Outcome::Error => "error",
            Outcome::Health => "health",
            Outcome::Aborted => "aborted",
            Outcome::Unmatched => "unmatched",
        }
    }

    /// Fallback when nothing recorded an outcome, e.g. axum rejections.
    fn from_status(status: StatusCode, has_hash: bool) -> Self {
        match status {
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED if !has_hash => {
                Outcome::Unmatched
            }
            StatusCode::UNAUTHORIZED => Outcome::Unauthorized,
            StatusCode::FORBIDDEN => Outcome::Forbidden,
            StatusCode::CONFLICT => Outcome::Exists,
            s if s.is_server_error() => Outcome::Error,
            _ => Outcome::Invalid,
        }
    }
}

#[derive(Debug, Default)]
struct RecordFields {
    hash: Option<String>,
    access: Access,
    outcome: Option<Outcome>,
    bytes: u64,
}

/// Filled in by inner layers, read by `request_log`.
#[derive(Clone, Debug, Default)]
pub struct RequestRecord(Arc<Mutex<RecordFields>>);

impl RequestRecord {
    fn fields(&self) -> MutexGuard<'_, RecordFields> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Validated hashes only; never log raw client input. Also records it on
    /// the current span, so call it inside the request span.
    pub fn set_hash(&self, hash: &str) {
        self.fields().hash = Some(hash.to_owned());
        Span::current().record("hash", hash);
    }

    pub fn set_access(&self, access: Access) {
        self.fields().access = access;
    }

    pub fn set_outcome(&self, outcome: Outcome) {
        self.fields().outcome = Some(outcome);
    }

    pub fn set_bytes(&self, bytes: u64) {
        self.fields().bytes = bytes;
    }
}

/// One line per request. `duration_ms` excludes streaming the response body.
pub async fn request_log(mut request: Request, next: Next) -> Response {
    let record = RequestRecord::default();
    request.extensions_mut().insert(record.clone());

    let mut line = RequestLine {
        record,
        method: request.method().clone(),
        started: Instant::now(),
        status: None,
    };
    let span = tracing::info_span!("request", method = %line.method, hash = tracing::field::Empty);

    let response = next.run(request).instrument(span).await;

    line.status = Some(response.status());
    response
}

/// Logs the request line on drop. No status means the client went away
/// before a response.
struct RequestLine {
    record: RequestRecord,
    method: Method,
    started: Instant,
    status: Option<StatusCode>,
}

impl Drop for RequestLine {
    fn drop(&mut self) {
        let duration_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let fields = std::mem::take(&mut *self.record.fields());
        let status = self.status;
        let outcome = match status {
            None => Outcome::Aborted,
            Some(status) => fields
                .outcome
                .unwrap_or_else(|| Outcome::from_status(status, fields.hash.is_some())),
        };
        let access = fields.access.as_str();
        let hash = fields.hash.as_deref();
        let bytes = fields.bytes;
        let method = &self.method;
        let status_code = status.map(|s| s.as_u16());

        // `event!` needs a const level.
        macro_rules! request_event {
            ($level:expr) => {
                tracing::event!(
                    $level,
                    method = %method,
                    hash,
                    status = status_code,
                    outcome = outcome.as_str(),
                    access,
                    bytes,
                    duration_ms,
                    "request"
                )
            };
        }

        // Routine traffic stays at DEBUG so the default level shows only what needs attention.
        if status.is_some_and(|s| s.is_server_error()) {
            request_event!(Level::ERROR);
        } else if matches!(outcome, Outcome::Unauthorized | Outcome::Invalid) {
            request_event!(Level::WARN);
        } else if outcome == Outcome::Aborted {
            request_event!(Level::INFO);
        } else {
            request_event!(Level::DEBUG);
        }
    }
}
