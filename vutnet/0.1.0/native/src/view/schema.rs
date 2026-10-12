//! Canonical HTML schema used by the Vut View validator and serializer boundary.

pub(super) const TAGS: &[&str] = &[
    "a",
    "article",
    "aside",
    "audio",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "code",
    "div",
    "em",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "html",
    "iframe",
    "img",
    "input",
    "label",
    "li",
    "link",
    "main",
    "meta",
    "nav",
    "ol",
    "option",
    "p",
    "pre",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strong",
    "style",
    "table",
    "tbody",
    "td",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "ul",
    "video",
];

pub(super) const BOOLEAN_ATTRIBUTES: &[&str] = &[
    "async",
    "autoplay",
    "checked",
    "controls",
    "defer",
    "disabled",
    "hidden",
    "loop",
    "multiple",
    "muted",
    "novalidate",
    "readonly",
    "required",
    "reversed",
    "selected",
];

pub(super) fn supports_tag(name: &str) -> bool {
    TAGS.contains(&name)
}

pub(super) fn is_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub(super) fn is_boolean_attribute(name: &str) -> bool {
    BOOLEAN_ATTRIBUTES.contains(&name)
}
