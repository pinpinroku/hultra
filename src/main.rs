use anyhow::Context;
use clap::Parser;
use tracing::{debug, info};

use crate::{
    cli::Cli,
    config::{AppConfig, CARGO_PKG_NAME, CARGO_PKG_VERSION},
};

mod archive;
mod cache;
mod cli;
mod commands;
mod config;
mod core;
mod dependency;
mod local_mods;
mod log;
mod mirror;
mod registry;
mod ui;
mod update;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    log::init_logger(args.log_file.as_deref()).with_context(|| {
        format!(
            "Failed to initialize logging system. Cannot create log file at {:?}",
            args.log_file.as_deref()
        )
    })?;

    info!("{} version {}", CARGO_PKG_NAME, CARGO_PKG_VERSION);
    debug!(?args);

    let config = AppConfig::new(args.directory.as_deref())?;

    cli::dispatch(args, config).await
}
