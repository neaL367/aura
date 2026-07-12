use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initializes the logging system using tracing-subscriber.
/// Logs are printed to stderr/stdout and filtered according to the `RUST_LOG` environment variable.
/// Default fallback log level is "info".
pub fn init() {
    let fmt_layer = fmt::layer()
        .with_thread_ids(true)
        .with_target(true);

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info,aura=debug"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
