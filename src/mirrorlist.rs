//! Mirror URL handling for GameBanana mod downloads.

use tracing::instrument;

use crate::cli::Mirror;

const EXPECTED_PREFIXES: [&str; 4] = [
    "http://gamebanana.com/dl/",
    "https://gamebanana.com/dl/", // NOTE This is a valid prefix of manual download link in the GameBanana page.
    "http://gamebanana.com/mmdl/",
    "https://gamebanana.com/mmdl/", // NOTE Currently, this is the only valid prefix as a download link in the remote registry.
];

/// Retrieves a list of mirror URLs for a given GameBanana URL, based on a slice of mirror priority.
#[instrument]
pub async fn generate_mirrors(download_url: &str, mirror_priority: &[Mirror]) -> Vec<String> {
    let Some(gbid) = extract_gamebanana_id(download_url).await else {
        return vec![download_url.to_string()];
    };

    mirror_priority
        .iter()
        .map(|preference| preference.url_for_id(gbid))
        .collect()
}

/// Extracts a GameBanana ID from a given URL.
async fn extract_gamebanana_id(url: &str) -> Option<u32> {
    for prefix in EXPECTED_PREFIXES {
        if let Some(id_str) = url.strip_prefix(prefix)
            && let Ok(id) = id_str.parse::<u32>()
        {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests_mirrorlist {
    use super::*;

    use anyhow::Context;

    #[tokio::test]
    async fn test_extract_gamebanana_id_success() -> anyhow::Result<()> {
        let urls = vec![
            "http://gamebanana.com/dl/123",
            "https://gamebanana.com/dl/456",
            "http://gamebanana.com/mmdl/789",
            "https://gamebanana.com/mmdl/101112",
        ];
        let expected_ids = vec![123, 456, 789, 101112];

        for (url, expected_id) in urls.into_iter().zip(expected_ids) {
            let id = extract_gamebanana_id(url)
                .await
                .with_context(|| format!("Failed to extract ID from URL: {}", url))?;
            assert_eq!(id, expected_id, "ID extracted from {} was incorrect", url);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_extract_gamebanana_id_failure() {
        let invalid_urls = vec![
            "https://gamebanana.com/mods/123", // Wrong prefix
            "https://example.com/dl/123",      // Wrong domain
            "https://gamebanana.com/dl/abc",   // Non-numeric ID
            "https://gamebanana.com/dl/",      // Missing ID
            "",                                // Empty string
        ];

        for url in invalid_urls {
            let result = extract_gamebanana_id(url).await;
            assert!(result.is_none(), "Expected an error for URL: {}", url);
        }
    }

    #[tokio::test]
    async fn test_generate_mirrors() {
        let mirror_priority = vec![Mirror::Otobot, Mirror::Gb, Mirror::Wegfan];

        let url = "https://gamebanana.com/mmdl/12345";
        let mirrors = generate_mirrors(url, &mirror_priority).await;

        let expected_mirrors = vec![
            "https://banana-mirror-mods.celestemods.com/12345.zip",
            "https://gamebanana.com/mmdl/12345",
            "https://celeste.weg.fan/api/v2/download/gamebanana-files/12345",
        ];

        assert_eq!(mirrors, expected_mirrors);
    }
}
