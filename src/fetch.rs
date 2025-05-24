use indicatif::ProgressBar;
use serde::de::DeserializeOwned;

use crate::error::Error;

/// Fetches the remote data from the given URL and parses it into the specified type.
pub async fn fetch_remote_data<T>(url: &str, msg: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let spinner = create_spinner(msg);

    let client = reqwest::ClientBuilder::new()
        .http2_prior_knowledge()
        .gzip(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let response = client.get(url).send().await?.error_for_status()?;

    tracing::debug!("Response headers: {:#?}", response.headers());
    let bytes = response.bytes().await?;

    spinner.finish_and_clear();

    tracing::info!("Parsing the binary data from the response");
    let data = parse_response_bytes::<T>(&bytes)?;

    Ok(data)
}

/// Parses binary data from the response into the specified type.
fn parse_response_bytes<T>(bytes: &[u8]) -> Result<T, serde_yaml_ng::Error>
where
    T: DeserializeOwned,
{
    serde_yaml_ng::from_slice::<T>(bytes)
}

/// Creates a spinner with specified message.
fn create_spinner(msg: &str) -> ProgressBar {
    use indicatif::ProgressStyle;
    use std::time::Duration;

    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.bold} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    spinner.set_message(format!("Fetching {}...", msg));
    spinner
}

#[cfg(test)]
mod tests_fetch {
    use crate::fetch::parse_response_bytes;
    use crate::mod_registry::RemoteModRegistry;

    #[test]
    fn test_parse_response_bytes_valid() {
        // Real example of SpeedrunTool
        let yaml = r#"
SpeedrunTool:
  GameBananaType: Tool
  Version: 3.24.3
  LastUpdate: 1739450250
  Size: 251301
  GameBananaId: 6597
  GameBananaFieldId: 1380853
  xxHash:
  - cbc55c04533efb34
  URL: "https://gamebanana.com/mmdl/1380853"
"#;

        let bytes = yaml.as_bytes();
        let result = parse_response_bytes(bytes);
        assert!(result.is_ok());
        let registry: RemoteModRegistry = result.unwrap();
        assert!(registry.contains_key("SpeedrunTool"));
    }
}
