//! Validation policy for Vut View tags, attributes, and URL-valued attributes.

use std::collections::HashSet;

use crate::view::{RenderError, css, schema};

pub(super) fn tag(name: &str) -> Result<(), RenderError> {
    if schema::supports_tag(name) {
        Ok(())
    } else {
        Err(invalid("unsupported HTML element name"))
    }
}

pub(super) fn is_void_tag(name: &str) -> bool {
    schema::is_void_tag(name)
}

pub(super) fn attributes(
    entries: impl IntoIterator<Item = (String, String, bool, bool, bool)>,
) -> Result<Vec<(String, String)>, RenderError> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    let mut style_index = None;
    let mut style_declarations = 0;
    for (name, value, is_boolean, enabled, is_css) in entries {
        if is_css {
            if is_boolean || !enabled || !css::valid_declaration(&name, &value) {
                return Err(invalid("invalid CSS property or value"));
            }
            let index = if let Some(index) = style_index {
                index
            } else {
                if !seen.insert("style".to_owned()) {
                    return Err(invalid("duplicate HTML attribute"));
                }
                output.push(("style".to_owned(), String::new()));
                let index = output.len() - 1;
                style_index = Some(index);
                index
            };
            if style_declarations > 0 {
                output[index].1.push_str("; ");
            }
            output[index].1.push_str(&name);
            output[index].1.push_str(": ");
            output[index].1.push_str(&value);
            style_declarations += 1;
            continue;
        }
        if !valid_attribute_name(&name) {
            return Err(invalid("invalid HTML attribute name"));
        }
        let normalized = name.to_ascii_lowercase();
        if normalized.starts_with("on") {
            return Err(invalid("event attributes are not supported"));
        }
        if normalized == "style" {
            return Err(invalid("use the typed `css` map for inline styles"));
        }
        if !seen.insert(normalized.clone()) {
            return Err(invalid("duplicate HTML attribute"));
        }
        if is_boolean {
            if !schema::is_boolean_attribute(&normalized) {
                return Err(invalid("unknown HTML boolean attribute"));
            }
            if enabled {
                output.push((normalized, String::new()));
            }
            continue;
        }
        if schema::is_boolean_attribute(&normalized) {
            return Err(invalid("HTML boolean attributes require a boolean value"));
        }
        if is_url_attribute(&normalized) && !safe_url(&value) {
            return Err(invalid("unsafe URL attribute value"));
        }
        output.push((normalized, value));
    }
    Ok(output)
}

fn valid_attribute_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn is_url_attribute(name: &str) -> bool {
    matches!(name, "href" | "src" | "action" | "formaction" | "poster")
}

fn safe_url(value: &str) -> bool {
    if value
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let Some(colon) = value.find(':') else {
        return true;
    };
    let delimiter = value.find(['/', '?', '#']).unwrap_or(usize::MAX);
    if colon > delimiter {
        return true;
    }
    let scheme = value[..colon].to_ascii_lowercase();
    matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel")
}

fn invalid(message: &str) -> RenderError {
    RenderError::Invalid(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{attributes, is_void_tag, safe_url, tag};

    #[test]
    fn only_supported_tags_and_void_elements_are_accepted() {
        assert!(tag("div").is_ok());
        assert!(tag("iframe").is_ok());
        assert!(tag("textarea").is_ok());
        assert!(tag("script").is_ok());
        assert!(is_void_tag("br"));
        assert!(is_void_tag("meta"));
        assert!(is_void_tag("source"));
        assert!(!is_void_tag("div"));
    }

    #[test]
    fn attributes_reject_duplicates_events_style_and_unsafe_urls() {
        assert!(attributes([("class".into(), "card".into(), false, true, false)]).is_ok());
        assert!(
            attributes([
                ("class".into(), "a".into(), false, true, false),
                ("CLASS".into(), "b".into(), false, true, false),
            ])
            .is_err()
        );
        assert!(attributes([("onload".into(), "alert(1)".into(), false, true, false)]).is_err());
        assert!(attributes([("style".into(), "color:red".into(), false, true, false)]).is_err());
        assert!(
            attributes([(
                "href".into(),
                "javascript:alert(1)".into(),
                false,
                true,
                false,
            )])
            .is_err()
        );
    }

    #[test]
    fn relative_and_allowlisted_absolute_urls_are_accepted() {
        assert!(safe_url("/docs?q=1"));
        assert!(safe_url("#section"));
        assert!(safe_url("https://example.test/path"));
        assert!(!safe_url("data:text/html,evil"));
        assert!(!safe_url("java\nscript:alert(1)"));
    }

    #[test]
    fn disabled_boolean_attributes_are_omitted() {
        assert_eq!(
            attributes([("hidden".into(), String::new(), true, false, false)]).unwrap(),
            Vec::<(String, String)>::new()
        );
        assert!(matches!(
            attributes([("hidden".into(), String::new(), true, true, false)]),
            Ok(values) if values == [("hidden".to_owned(), String::new())]
        ));
    }

    #[test]
    fn custom_data_and_aria_attributes_are_allowed_but_boolean_types_are_checked() {
        assert!(attributes([("data-page".into(), "home".into(), false, true, false)]).is_ok());
        assert!(attributes([("aria-label".into(), "Profile".into(), false, true, false)]).is_ok());
        assert!(attributes([("disabled".into(), "false".into(), false, true, false)]).is_err());
        assert!(attributes([("role".into(), String::new(), true, true, false)]).is_err());
    }
}
