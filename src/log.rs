use std::{fs::File, path::Path};

use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub fn init_logger(log_file: Option<&Path>, verbose: bool, quiet: bool) {
    // 1. level config for stderr
    let console_level = if quiet {
        "error"
    } else if verbose {
        "hultra=debug,info" // debug for my app, info for others
    } else {
        "hultra=info,warn" // info for my app, warn for others
    };

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .without_time()
        .with_filter(EnvFilter::new(console_level));

    // 2. layer for file output
    let file_layer = log_file.map(|path| {
        let f = File::create(path).unwrap_or_else(|e| {
            eprintln!(
                "Error: failed to create the log file '{}': {}",
                path.display(),
                e
            );
            std::process::exit(1);
        });

        fmt::layer()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_writer(f)
            .with_ansi(false)
            .with_filter(EnvFilter::new("hultra=debug,info"))
    });

    // 3. register by merging them all
    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();
}
