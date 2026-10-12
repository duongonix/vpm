//! Validation for HTML raw-text elements whose children serialize as text.

use crate::view::{RenderError, serialize};

pub(super) fn validate(nodes: &[serialize::Node]) -> Result<(), RenderError> {
    for node in nodes {
        let serialize::Node::Element(element) = node else {
            continue;
        };

        if element.name.local.as_ref() == "style" {
            validate_style_children(&element.children)?;
        } else {
            validate(&element.children)?;
        }
    }
    Ok(())
}

fn validate_style_children(children: &[serialize::Node]) -> Result<(), RenderError> {
    let mut css = String::new();
    for child in children {
        match child {
            serialize::Node::Text(text) => css.push_str(text),
            serialize::Node::Element(_) | serialize::Node::Comment(_) => {
                return Err(invalid("style elements may contain text nodes only"));
            }
        }
    }

    if css
        .as_bytes()
        .windows(b"</style".len())
        .any(|sequence| sequence.eq_ignore_ascii_case(b"</style"))
    {
        return Err(invalid("style content cannot contain a `</style` sequence"));
    }

    Ok(())
}

fn invalid(message: &str) -> RenderError {
    RenderError::Invalid(message.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::view::{raw_text, serialize};

    fn style(children: Vec<serialize::Node>) -> serialize::Node {
        serialize::Node::Element(serialize::Element {
            name: serialize::element_name("style"),
            attributes: Vec::new(),
            children,
        })
    }

    #[test]
    fn style_css_is_preserved_as_raw_text() {
        let css = ".app > .title { content: \"徽 & <\"; }";
        let tree = [style(vec![serialize::Node::Text(css.to_owned())])];

        raw_text::validate(&tree).unwrap();
        assert_eq!(
            serialize::html(&tree).unwrap(),
            format!("<style>{css}</style>")
        );
    }

    #[test]
    fn empty_style_serializes_and_closing_style_sequences_are_rejected() {
        let empty = [style(Vec::new())];
        raw_text::validate(&empty).unwrap();
        assert_eq!(serialize::html(&empty).unwrap(), "<style></style>");

        for css in ["a { content: '</style>'; }", "a { content: '</STYLE >'; }"] {
            let tree = [style(vec![serialize::Node::Text(css.to_owned())])];
            assert!(raw_text::validate(&tree).is_err(), "accepted {css:?}");
        }
    }

    #[test]
    fn closing_style_sequence_across_text_nodes_is_rejected() {
        let tree = [style(vec![
            serialize::Node::Text("before</sty".to_owned()),
            serialize::Node::Text("le><script>".to_owned()),
        ])];

        assert!(raw_text::validate(&tree).is_err());
    }

    #[test]
    fn style_does_not_accept_element_children_as_css_text() {
        let tree = [style(vec![serialize::Node::Element(serialize::Element {
            name: serialize::element_name("em"),
            attributes: Vec::new(),
            children: Vec::new(),
        })])];

        assert!(raw_text::validate(&tree).is_err());
    }
}
