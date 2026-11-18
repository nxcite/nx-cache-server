use crate::domain::config::LogFormat;
use tracing_subscriber::{
    filter::{ParseError, Targets},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

/// `DEBUG` without `RUST_LOG`: our crates at debug, the rest at info.
const DEBUG_FILTER: &str = "nx_cache_server=debug,nx_cache_aws=debug,info";

const DEFAULT_FILTER: &str = "info";

#[derive(Debug)]
pub struct LogFilter {
    pub targets: Targets,
    /// `DEBUG` was set but `RUST_LOG` won.
    pub debug_ignored: bool,
}

/// `RUST_LOG` wins over `DEBUG`; an empty `RUST_LOG` counts as unset.
pub fn resolve_filter(rust_log: Option<&str>, debug: bool) -> Result<LogFilter, ParseError> {
    match rust_log.filter(|value| !value.is_empty()) {
        Some(value) => Ok(LogFilter {
            targets: value.parse()?,
            debug_ignored: debug,
        }),
        None => {
            let spec = if debug { DEBUG_FILTER } else { DEFAULT_FILTER };
            Ok(LogFilter {
                targets: spec.parse()?,
                debug_ignored: false,
            })
        }
    }
}

pub fn init(format: LogFormat, targets: Targets) {
    let layer = match format {
        LogFormat::Text => fmt::layer().boxed(),
        LogFormat::Json => fmt::layer()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .boxed(),
    };

    tracing_subscriber::registry()
        .with(layer.with_filter(targets))
        .init();
}
