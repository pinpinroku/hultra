use std::{
    env,
    fs::{self, File},
    io,
    path::PathBuf,
};

use tracing_subscriber::fmt::format::FmtSpan;

const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(thiserror::Error, Debug)]
enum LogError {
    #[error("failed to determine a XDG base directory")]
    DetermineXdgBaseDirectory,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn set_up_logger(verbose: bool) {
    let log_level = if verbose {
        format!("{}=debug", APP_NAME)
    } else {
        format!("{}=info", APP_NAME)
    };

    match create_log_file() {
        Ok(writer) => tracing_subscriber::fmt()
            .compact()
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter(log_level)
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(false)
            .with_writer(writer)
            .with_ansi(false)
            .init(),
        Err(err) => {
            eprintln!("WARN failed to create log file: cause {}", err);
            eprintln!("INFO sending log to stderr");
            tracing_subscriber::fmt()
                .compact()
                .with_span_events(FmtSpan::CLOSE)
                .with_env_filter(log_level)
                .with_file(false)
                .with_line_number(false)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_target(false)
                .init()
        }
    }
}

fn create_log_file() -> Result<File, LogError> {
    let state_home = if let Ok(val) = env::var("XDG_STATE_HOME") {
        PathBuf::from(val)
    } else if let Some(home) = env::home_dir() {
        home.join(".local").join("state")
    } else {
        return Err(LogError::DetermineXdgBaseDirectory);
    };

    let log_dir = state_home.join(APP_NAME);

    fs::create_dir_all(&log_dir)?;

    let path = log_dir.join(APP_NAME).with_extension("log");
    let writer = File::create(&path)?;

    Ok(writer)
}
