use clap::ValueEnum;

/// Supported mirrors.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum Mirror {
    /// Default GameBanana Server (United States).
    Gb,
    /// Germany.
    Jade,
    /// China.
    Wegfan,
    /// North America.
    Otobot,
}

impl Mirror {
    /// Generates the full mirror URL for a given GameBanana ID.
    pub fn url_for_id(&self, gbid: u32) -> String {
        match self {
            Mirror::Gb => {
                format!("https://gamebanana.com/mmdl/{}", gbid)
            }
            Mirror::Jade => {
                format!(
                    "https://celestemodupdater.0x0a.de/banana-mirror/{}.zip",
                    gbid
                )
            }
            Mirror::Wegfan => {
                format!(
                    "https://celeste.weg.fan/api/v2/download/gamebanana-files/{}",
                    gbid
                )
            }
            Mirror::Otobot => {
                format!("https://banana-mirror-mods.celestemods.com/{}.zip", gbid)
            }
        }
    }
}
