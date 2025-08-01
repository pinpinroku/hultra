use std::borrow::Cow;

/// Gets all mirror URLs based on the given preferences.
///
/// The following code is my Rust implementation, ported from the original C# code. I extend my respect to the original author(s) for their work.
///
/// Make sure to keep this in sync with
/// - https://github.com/EverestAPI/Everest/blob/dev/Celeste.Mod.mm/Mod/Helpers/ModUpdaterHelper.cs :: getAllMirrorUrls
/// - https://github.com/EverestAPI/Olympus/blob/main/sharp/CmdUpdateAllMods.cs :: getAllMirrorUrls
/// - https://github.com/maddie480/RandomStuffWebsite/blob/main/front-vue/src/components/ModListItem.vue :: getMirrorLink
///
///
/// # Example
/// ```rust
/// use get_all_mirrors::get_all_mirror_urls;
///
/// for url in get_all_mirror_urls("https://gamebanana.com/dl/12345", "jade,gb") {
///     println!("{}", url);
/// }
/// ```
pub fn get_all_mirror_urls<'a>(url: &'a str, mirror_preferences: &str) -> Vec<Cow<'a, str>> {
    let gbid = extract_gamebanana_id(url);

    if gbid == 0 {
        return vec![Cow::Borrowed(url)];
    }

    mirror_preferences
        .split(',')
        .filter_map(|mirror_id| match mirror_id.trim() {
            "gb" => Some(Cow::Borrowed(url)),
            "jade" => Some(Cow::Owned(format!(
                "https://celestemodupdater.0x0a.de/banana-mirror/{gbid}.zip"
            ))),
            "wegfan" => Some(Cow::Owned(format!(
                "https://celeste.weg.fan/api/v2/download/gamebanana-files/{gbid}"
            ))),
            "otobot" => Some(Cow::Owned(format!(
                "https://banana-mirror-mods.celestemods.com/{gbid}.zip"
            ))),
            _ => None,
        })
        .collect()
}

/// Extracts gemebanana ID from given URL.
fn extract_gamebanana_id(url: &str) -> u32 {
    let prefixes = [
        "http://gamebanana.com/dl/",
        "https://gamebanana.com/dl/",
        "http://gamebanana.com/mmdl/",
        "https://gamebanana.com/mmdl/",
    ];

    for prefix in &prefixes {
        if let Some(id_str) = url.strip_prefix(prefix) {
            if let Ok(id) = id_str.parse::<u32>() {
                return id;
            }
        }
    }

    0 // Returns 0 if the extraction fails.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_gamebanana_id() {
        assert_eq!(
            extract_gamebanana_id("https://gamebanana.com/dl/12345"),
            12345
        );
        assert_eq!(
            extract_gamebanana_id("http://gamebanana.com/mmdl/67890"),
            67890
        );
        assert_eq!(extract_gamebanana_id("https://example.com/file"), 0);
    }

    #[test]
    fn test_mirror_urls() {
        let url = "https://gamebanana.com/dl/12345";
        let preferences = "jade,gb";
        let urls: Vec<Cow<'_, str>> = get_all_mirror_urls(url, preferences);

        assert_eq!(urls.len(), 2);
        assert_eq!(
            urls[0],
            "https://celestemodupdater.0x0a.de/banana-mirror/12345.zip"
        );
        assert_eq!(urls[1], "https://gamebanana.com/dl/12345");
    }
}
