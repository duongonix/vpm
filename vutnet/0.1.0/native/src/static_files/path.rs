//! Decode URL components once; never interpret an untrusted URL as an OS path.

use std::path::PathBuf;

use crate::request::decode_component;

pub(super) fn prefix(value: &str) -> Result<String, ()> {
    if !value.starts_with('/')
        || value.contains(['\\', '%', '?', '#', ':'])
        || value.bytes().any(|byte| byte.is_ascii_control())
        || value.contains("//")
    {
        return Err(());
    }
    if value.split('/').any(|part| part == "." || part == "..") {
        return Err(());
    }
    let trimmed = value.trim_end_matches('/');
    Ok(if trimmed.is_empty() {
        "/".into()
    } else {
        trimmed.into()
    })
}

pub(super) fn relative(value: &str) -> Result<PathBuf, ()> {
    let mut path = PathBuf::new();
    if value.is_empty() {
        return Ok(path);
    }
    for encoded in value.split('/') {
        let part = decode_component(encoded)?;
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains(['/', '\\', ':', '\0'])
            || part.ends_with(['.', ' '])
            || part.chars().any(char::is_control)
            || reserved_windows_name(&part)
        {
            return Err(());
        }
        // Every appended component is a normal relative name on both platforms.
        path.push(part);
    }
    Ok(path)
}

fn reserved_windows_name(part: &str) -> bool {
    let base = part
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            matches!(
                base.strip_prefix(prefix),
                Some("0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
            )
        })
}
