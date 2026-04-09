/// Sanitizes a mod name as file stem for Unix file systems.
///
/// # Rules
/// - Trims leading/trailing whitespace.
/// - Removes control characters.
/// - Replaces characters not in the whitelist `[A-Za-z0-9 -_'()]` with `_`.
/// - Truncates the result to 255 bytes.
///
/// # Panics
/// All characters in given string must be ASCII, otherwise it will panic.
///
/// # Notes
/// Mod database only allows ASCII characters for the mod name. So the name should always valid UTF-8 and ASCII.
pub fn sanitize_stem(input: &str) -> String {
    let trimmed = input.trim();

    assert!(
        trimmed.is_ascii(),
        "Input string should contains only ASCII characters"
    );

    let sanitized_bytes = trimmed
        .bytes()
        .filter(|c| !c.is_ascii_control())
        .map(|c| {
            if c.is_ascii_alphanumeric() || is_allowed_byte(c) {
                c
            } else {
                b'_'
            }
        })
        .take(u8::MAX as usize)
        .collect();

    // NOTE This is safe because `input` is always valid UFT-8 and ASCII
    unsafe { String::from_utf8_unchecked(sanitized_bytes) }
}

/// Checks if a byte is allowed in the filename stem.
#[inline(always)]
fn is_allowed_byte(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z' |            // Uppercase
        b'a'..=b'z' |            // Lowercase
        b'0'..=b'9' |            // Digits
        b' ' | b'-' | b'_' |     // Separators
        b'\'' | b'(' | b')' |    // Special allowed chars
        b'+' | b','              // Special allowed chars 2 (common in mods name)
    )
}

#[cfg(test)]
mod test_sanitize_name {
    use super::*;

    #[tokio::test]
    async fn test_no_change() {
        let input = "valid-filename_123(final)";
        let result = sanitize_stem(input);
        assert_eq!(result, "valid-filename_123(final)");
    }

    #[tokio::test]
    async fn test_replace_invalid_chars() {
        let input = "file!?.txt";
        let result = sanitize_stem(input);
        assert_eq!(result, "file___txt");
    }

    #[tokio::test]
    async fn test_remove_control_chars() {
        // Control chars should be removed, not replaced
        let input = "file\0name\n";
        let result = sanitize_stem(input);
        assert_eq!(result, "filename");
    }

    #[tokio::test]
    async fn test_mixed_whitelist() {
        // Ensure added whitelist chars ' and () are respected
        let input = "  Spooooky's Asset Pack (WIP)  ";
        let result = sanitize_stem(input);
        assert_eq!(result, "Spooooky's Asset Pack (WIP)");
    }

    #[tokio::test]
    #[should_panic(expected = "Input string should contains only ASCII characters")]
    async fn test_panic_on_non_ascii() {
        sanitize_stem("Error_日本語");
    }
}

/// Gets first 19 charcters from "2026-03-07T19:48:53.0343351Z", replace 'T' with ' '
pub fn format_date(date: &str) -> String {
    date.get(0..19)
        .map(|s| s.replace('T', " "))
        .unwrap_or_else(|| date.to_string())
}

#[cfg(test)]
mod test_format_date {
    use super::*;

    #[test]
    fn test_expected_variations() {
        let cases = vec![
            (
                "2026-03-07T19:48:53.034Z",
                "2026-03-07 19:48:53",
                "Long ISO",
            ),
            ("2026-03-07T19:48:53Z", "2026-03-07 19:48:53", "Short ISO"),
            (
                "2026-03-07 19:48:53",
                "2026-03-07 19:48:53",
                "Already formatted",
            ),
            ("invalid-date", "invalid-date", "Invalid format"),
        ];

        for (input, expected, description) in cases {
            assert_eq!(
                format_date(input),
                expected,
                "Failed on case: {}",
                description
            );
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid checksum: could not parse the '{input}' with digits in base 16")]
pub struct ChecksumError {
    input: String,
    #[source]
    source: std::num::ParseIntError,
}

pub fn from_str_digest(input: &str) -> Result<u64, ChecksumError> {
    let clean_input = input.trim().strip_prefix("0x").unwrap_or(input.trim());
    u64::from_str_radix(clean_input, 16).map_err(|err| ChecksumError {
        input: input.to_string(),
        source: err,
    })
}

#[cfg(test)]
mod tests_from_str_digest {
    use super::from_str_digest;

    use anyhow::{Context, Result};

    #[test]
    fn parses_with_0x() -> Result<()> {
        assert_eq!(
            from_str_digest("0x7f4d96733b93c52c").context("should be converted")?,
            9173153437688513836
        );
        Ok(())
    }

    #[test]
    fn parses_without_0x_and_spaces() -> Result<()> {
        assert_eq!(
            from_str_digest(" 7f4d96733b93c52c ").context("should be converted")?,
            9173153437688513836
        );
        Ok(())
    }

    #[test]
    fn returns_error_on_invalid() {
        let err = from_str_digest("not-hex").unwrap_err();
        assert!(format!("{}", err).contains("invalid checksum"));
    }
}
