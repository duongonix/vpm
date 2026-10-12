//! HTML5ever-backed serialization of the validated Vut View tree.

use std::io;

use html5ever::{LocalName, QualName, namespace_url, ns};
use markup5ever::serialize::{Serialize, Serializer, TraversalScope};

pub(super) struct Element {
    pub(super) name: QualName,
    pub(super) attributes: Vec<(QualName, String)>,
    pub(super) children: Vec<Node>,
}

pub(super) enum Node {
    Element(Element),
    Text(String),
    Comment(String),
}

pub(super) fn html(nodes: &[Node]) -> Result<String, String> {
    let mut output = Vec::new();
    let mut serializer = html5ever::serialize::HtmlSerializer::new(
        &mut output,
        html5ever::serialize::SerializeOpts::default(),
    );
    for node in nodes {
        node.serialize(&mut serializer, TraversalScope::IncludeNode)
            .map_err(|_| "HTML serialization failed".to_owned())?;
    }
    String::from_utf8(output).map_err(|_| "HTML serializer returned invalid UTF-8".to_owned())
}

impl Serialize for Node {
    fn serialize<S>(&self, serializer: &mut S, _scope: TraversalScope) -> io::Result<()>
    where
        S: Serializer,
    {
        match self {
            Self::Text(text) => serializer.write_text(text),
            Self::Comment(comment) => serializer.write_comment(comment),
            Self::Element(element) => {
                serializer.start_elem(
                    element.name.clone(),
                    element
                        .attributes
                        .iter()
                        .map(|(name, value)| (name, value.as_str())),
                )?;
                for child in &element.children {
                    child.serialize(serializer, TraversalScope::IncludeNode)?;
                }
                serializer.end_elem(element.name.clone())
            }
        }
    }
}

pub(super) fn element_name(name: &str) -> QualName {
    QualName::new(None, ns!(html), LocalName::from(name))
}

pub(super) fn attribute_name(name: &str) -> QualName {
    QualName::new(None, ns!(), LocalName::from(name))
}

#[cfg(test)]
mod tests {
    use super::{Element, Node, attribute_name, element_name, html};

    #[test]
    fn stylesheet_link_serializes_as_an_escaped_void_element() {
        let tree = [Node::Element(Element {
            name: element_name("link"),
            attributes: vec![
                (attribute_name("rel"), "stylesheet".to_owned()),
                (attribute_name("href"), "/styles/app.css".to_owned()),
                (
                    attribute_name("media"),
                    "screen & (min-width: \"768px\")".to_owned(),
                ),
            ],
            children: Vec::new(),
        })];

        assert_eq!(
            html(&tree).unwrap(),
            "<link rel=\"stylesheet\" href=\"/styles/app.css\" media=\"screen &amp; (min-width: &quot;768px&quot;)\">"
        );
    }

    #[test]
    fn call_site_comments_are_serialized_without_changing_text_content() {
        let tree = [
            Node::Comment("vut-site:v1-0123456789abcdef".to_owned()),
            Node::Text("a<&b".to_owned()),
        ];

        assert_eq!(
            html(&tree).unwrap(),
            "<!--vut-site:v1-0123456789abcdef-->a&lt;&amp;b"
        );
    }
}
