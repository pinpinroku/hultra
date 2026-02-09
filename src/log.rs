use std::{fs::File, io, path::{Component, Path}};

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
        .with_target(false)
        .without_time()
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

/// Swaps user's home direcotry path with tilde.
pub fn anonymize(path: &Path) -> String {
    // 1. trying to detect home dir from env var
    if let Some(home) = std::env::home_dir()
        && let Ok(rel) = path.strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }

    // 2. trying to guess it from path structure
    let mut comps = path.components();
    let root = comps.next();
    let base = comps.next();
    let user = comps.next();

    match (root, base, user) {
        (Some(Component::RootDir), Some(b), Some(_)) 
            // NOTE prevent /etc/systemd/system becomes ~/system
            if b.as_os_str() == "home" || b.as_os_str() == "Users" => 
        {
            let rest = comps.as_path();
            if rest.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rest.display())
            }
        }
        // 3. last resort: fallback to original
        _ => path.to_string_lossy().into_owned(),
    }
}
