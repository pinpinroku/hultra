//! Interface design
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

/// Create a progress bar for downloading a file.
pub fn create_download_progress_bar(name: &str) -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.set_style(
        ProgressStyle::with_template(
            "{wide_msg} {total_bytes:>10.1.cyan/blue} {bytes_per_sec:>11.2} {elapsed_precise:>8} [{bar:>40}] {percent:>3}%"
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-")
    );
    pb.set_message(name.to_string());
    pb
}

/// Create a spinner progress bar for fetching online database.
pub fn create_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.bold} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    spinner.set_message("fetching database...");
    spinner
}
