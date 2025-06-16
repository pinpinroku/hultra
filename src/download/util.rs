use std::borrow::Cow;

/// Returns sanitized mod name or "unnamed" if the given mod name is empty.
///
/// This function replaces any invalid characters with underscores, trims whitespace,
/// and ensures the resulting string does not exceed 255 characters.
pub fn sanitize(mod_name: &str) -> Cow<'_, str> {
    const BAD_CHARS: [char; 6] = ['/', '\\', '*', '?', ':', ';'];

    let trimmed = mod_name.trim();
    let without_dot = trimmed.strip_prefix('.').unwrap_or(trimmed);

    let mut changed = false;
    let mut result = String::with_capacity(without_dot.len());

    for c in without_dot
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
    {
        let replacement = match c {
            '\r' | '\n' | '\0' => {
                changed = true;
                continue;
            }
            c if BAD_CHARS.contains(&c) => {
                changed = true;
                '_'
            }
            c => c,
        };
        result.push(replacement);
    }

    if result.len() > 255 {
        result.truncate(255);
        changed = true;
    }

    if result.is_empty() {
        Cow::Borrowed("unnamed")
    } else if !changed && result == mod_name {
        Cow::Borrowed(mod_name)
    } else {
        Cow::Owned(result)
    }
}

#[cfg(test)]
mod tests_sanitize {
    use super::*;

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize(""), Cow::Borrowed("unnamed"));
    }

    #[test]
    fn test_sanitize_with_bad_chars() {
        assert_eq!(
            sanitize("Mod/Name*With?Bad:Chars;"),
            Cow::Owned::<str>("Mod_Name_With_Bad_Chars_".to_string())
        );
    }

    #[test]
    fn test_sanitize_with_whitespace() {
        assert_eq!(
            sanitize("  Mod Name  "),
            Cow::Owned::<str>("Mod Name".to_string())
        );
    }

    #[test]
    fn test_sanitize_long_name() {
        let long_name = "a".repeat(300);
        assert_eq!(sanitize(&long_name).len(), 255);
    }
}
