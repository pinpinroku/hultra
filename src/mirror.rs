//! Internal mirror definition.
use crate::commands::Mirror;

pub use mirrorlist::generate;

/// Supported mirrors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainMirror {
    Gb,     // US
    Jade,   // DE
    Wegfan, // CN
    Otobot, // NA
}

impl From<&Mirror> for DomainMirror {
    fn from(value: &Mirror) -> Self {
        match value {
            Mirror::Gb => Self::Gb,
            Mirror::Jade => Self::Jade,
            Mirror::Wegfan => Self::Wegfan,
            Mirror::Otobot => Self::Otobot,
        }
    }
}

impl DomainMirror {
    /// Generates the full mirror URL for a given GameBanana ID.
    fn url_for_id(&self, gbid: &str) -> String {
        match *self {
            DomainMirror::Gb => {
                format!("https://gamebanana.com/mmdl/{}", gbid)
            }
            DomainMirror::Jade => {
                format!(
                    "https://celestemodupdater.0x0a.de/banana-mirror/{}.zip",
                    gbid
                )
            }
            DomainMirror::Wegfan => {
                format!(
                    "https://celeste.weg.fan/api/v2/download/gamebanana-files/{}",
                    gbid
                )
            }
            DomainMirror::Otobot => {
                format!("https://banana-mirror-mods.celestemods.com/{}.zip", gbid)
            }
        }
    }
}

mod mirrorlist {
    use tracing::warn;

    use super::DomainMirror;

    pub fn generate(url: &str, priority: &[DomainMirror]) -> Vec<String> {
        let Some(gbid) = url.strip_prefix("https://gamebanana.com/mmdl/") else {
            warn!("failed to extract Gamebanana ID from '{}'", url);
            return vec![url.to_string()];
        };
        priority
            .iter()
            .map(|mirror| mirror.url_for_id(gbid))
            .collect()
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_generate() {
            let url = "https://gamebanana.com/mmdl/1298450";
            let result = generate(
                url,
                &[DomainMirror::Otobot, DomainMirror::Gb, DomainMirror::Jade],
            );
            assert_eq!(result.len(), 3, "should return three URLs");
            assert_eq!(
                result.first().unwrap(),
                &"https://banana-mirror-mods.celestemods.com/1298450.zip".to_string()
            )
        }
    }
}
