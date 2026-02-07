use std::{fs::File, io, path::Path};

use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub fn init_logger(log_file: Option<&Path>) -> Result<(), io::Error> {
    // if the variable `$RUST_LOG` is not set, do not display any logs to the console
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"));

    let console_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .pretty()
        .with_filter(env_filter);

    let file_layer = if let Some(p) = log_file {
        let file = File::create(p)?;

        Some(
            fmt::layer()
                .with_writer(file)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_ansi(false)
                .with_filter(EnvFilter::new("hultra=debug,info")), // always debug for file, info for deps
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    Ok(())
}
