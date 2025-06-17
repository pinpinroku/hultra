use reqwest::Url;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModPageUrlParseError {
    /// Invalid URL
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// Unsupported scheme
    #[error("unsupported scheme: {0} (expected 'http' or 'https')")]
    UnsupportedScheme(String),

    /// Invalid GameBanana URL
    #[error("invalid GameBanana URL :{0:?}")]
    InvalidGameBananaUrl(Url),

    /// Invalid Mod ID
    #[error("invalid Mod ID :{0} (expected unsigned 32 bit integer)")]
    InvalidModId(String),
}
