//! Read the typed Vut View schema through the public Native SDK and build a
//! validated `HTML5ever` serialization tree.

use vut_native_sdk::{Bool, Int, List, Nominal, Str, Value};

use crate::{
    identity::{ViewAttributeType, ViewRenderNodeType, ViewRenderType},
    view::{RenderError, hydration::PlanEntry, serialize, validate},
};

type Frame = (usize, serialize::Element);

pub(super) fn from_vut(
    view: &Value<Nominal<ViewRenderType>>,
    plan: Option<&[PlanEntry]>,
) -> Result<Vec<serialize::Node>, RenderError> {
    let nodes = view.get_field::<List<Nominal<ViewRenderNodeType>>>("_nodes")?;
    let include_call_site_markers = view
        .get_field::<Bool>("_include_call_site_markers")?
        .get()?;
    let include_bootstrap = view.get_field::<Bool>("_include_bootstrap")?.get()?;
    if include_call_site_markers && plan.is_none() {
        return Err(invalid("hydratable View is missing its shared plan"));
    }
    let mut roots = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();

    for index in 0..nodes.len()? {
        let node = nodes.get(index)?;
        let parent = node.get_field::<Int>("_parent")?.get()?;
        let parent_index = match parent {
            -1 => None,
            value if value < -1 => return Err(invalid("invalid View parent index")),
            value => {
                Some(usize::try_from(value).map_err(|_| invalid("invalid View parent index"))?)
            }
        };
        if parent_index.is_some_and(|parent_index| parent_index >= index) {
            return Err(invalid("invalid View parent index"));
        }
        close_to_parent(&mut stack, &mut roots, parent_index)?;

        let is_text = node.get_field::<Bool>("_is_text")?.get()?;
        let is_slot = node.get_field::<Bool>("_is_slot")?.get()?;
        if is_slot {
            return Err(invalid("unresolved View slot"));
        }
        let tag = read_str(&node, "_name")?;
        let text = read_str(&node, "_text")?;
        let attributes = read_attributes(&node)?;
        let plan_entry = plan.and_then(|entries| entries.get(index));
        let marker = if include_call_site_markers {
            Some(site_marker(plan_entry.ok_or_else(|| {
                invalid("View hydration plan is shorter than its node list")
            })?)?)
        } else {
            None
        };

        if is_text {
            if !tag.is_empty() || !attributes.is_empty() {
                return Err(invalid("malformed text node"));
            }
            if parent_preserves_comment_nodes(&stack)
                && let Some(marker) = marker.as_ref()
            {
                append_node(
                    &mut stack,
                    &mut roots,
                    serialize::Node::Comment(format!("vut-site:{marker}")),
                );
            }
            append_node(&mut stack, &mut roots, serialize::Node::Text(text));
            continue;
        }

        if !text.is_empty() {
            return Err(invalid("element node contains text payload"));
        }
        validate::tag(&tag)?;
        let mut attributes = attributes;
        if let Some(marker) = marker {
            if attributes.iter().any(|(name, _)| name == "data-vut-site") {
                return Err(invalid(
                    "`data-vut-site` is reserved for hydratable View output",
                ));
            }
            if include_bootstrap
                && attributes
                    .iter()
                    .any(|(name, _)| name == "data-vutnet-bootstrap")
            {
                return Err(invalid(
                    "`data-vutnet-bootstrap` is reserved for hydration output",
                ));
            }
            attributes.push(("data-vut-site".to_owned(), marker));
        }
        let attributes = attributes
            .into_iter()
            .map(|(name, value)| (serialize::attribute_name(&name), value))
            .collect();
        let element = serialize::Element {
            name: serialize::element_name(&tag),
            attributes,
            children: Vec::new(),
        };
        if validate::is_void_tag(&tag) {
            append_node(&mut stack, &mut roots, serialize::Node::Element(element));
        } else {
            stack.push((index, element));
        }
    }

    while let Some(frame) = stack.pop() {
        append_node(&mut stack, &mut roots, serialize::Node::Element(frame.1));
    }
    Ok(roots)
}

// HTML treats comment syntax as raw/RCDATA content in these elements, so a
// marker would become part of script, CSS, title, or textarea contents.
fn parent_preserves_comment_nodes(stack: &[Frame]) -> bool {
    !stack.last().is_some_and(|(_, element)| {
        matches!(
            element.name.local.as_ref(),
            "script" | "style" | "title" | "textarea"
        )
    })
}

fn site_marker(value: &PlanEntry) -> Result<String, RenderError> {
    if value.site_id == 0 {
        return Err(invalid("View node is missing its call-site identity"));
    }
    Ok(value.marker())
}

fn close_to_parent(
    stack: &mut Vec<Frame>,
    roots: &mut Vec<serialize::Node>,
    parent: Option<usize>,
) -> Result<(), RenderError> {
    let Some(parent) = parent else {
        while let Some(frame) = stack.pop() {
            append_node(stack, roots, serialize::Node::Element(frame.1));
        }
        return Ok(());
    };
    while stack.last().is_some_and(|(index, _)| *index != parent) {
        let Some(frame) = stack.pop() else {
            return Err(invalid("View parent is not an open element"));
        };
        if frame.0 < parent {
            return Err(invalid("View parent is not an ancestor"));
        }
        append_node(stack, roots, serialize::Node::Element(frame.1));
    }
    if stack.last().is_none() {
        return Err(invalid("View parent is not an open element"));
    }
    Ok(())
}

fn append_node(stack: &mut [Frame], roots: &mut Vec<serialize::Node>, node: serialize::Node) {
    if let Some((_, parent)) = stack.last_mut() {
        parent.children.push(node);
    } else {
        roots.push(node);
    }
}

fn read_attributes(
    node: &Value<Nominal<ViewRenderNodeType>>,
) -> Result<Vec<(String, String)>, RenderError> {
    let entries = node.get_field::<List<Nominal<ViewAttributeType>>>("_attrs")?;
    let mut values = Vec::new();
    for index in 0..entries.len()? {
        let attribute = entries.get(index)?;
        values.push((
            read_str(&attribute, "_name")?,
            read_str(&attribute, "_value")?,
            attribute.get_field::<Bool>("_is_boolean")?.get()?,
            attribute.get_field::<Bool>("_enabled")?.get()?,
            attribute.get_field::<Bool>("_is_css")?.get()?,
        ));
    }
    validate::attributes(values)
}

fn read_str<T: vut_native_sdk::NominalType>(
    value: &Value<Nominal<T>>,
    field: &str,
) -> Result<String, RenderError> {
    Ok(value.get_field::<Str>(field)?.as_str()?.to_owned())
}

fn invalid(message: &str) -> RenderError {
    RenderError::Invalid(message.to_owned())
}
