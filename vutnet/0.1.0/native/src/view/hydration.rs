//! Versioned SSR-to-WASM bootstrap payload and shared View plan validation.

use std::collections::HashMap;

use serde::Serialize;
use vut_native_sdk::{Bool, Int, List, Nominal, Str, U64, Value};

use crate::{
    identity::{ViewHydrationPlanEntryType, ViewRenderNodeType, ViewRenderType},
    view::{RenderError, serialize},
};

const BOOTSTRAP_PROTOCOL: &str = "vutnet.hydration";
const BOOTSTRAP_VERSION: u32 = 2;
const PLAN_VERSION: u32 = 2;
const MAX_BOOTSTRAP_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub(super) struct PlanEntry {
    pub(super) site: String,
    pub(super) occurrence: u64,
    pub(super) parent: i64,
    pub(super) kind: String,
    pub(super) name: String,
    #[serde(skip)]
    pub(super) site_id: u64,
}

impl PlanEntry {
    pub(super) fn marker(&self) -> String {
        format!("v2-{:016x}-{}", self.site_id, self.occurrence)
    }
}

#[derive(Serialize)]
struct Plan<'a> {
    version: u32,
    nodes: &'a [PlanEntry],
}

#[derive(Serialize)]
struct BootstrapEnvelope<'a> {
    protocol: &'static str,
    version: u32,
    page_id: String,
    has_data: bool,
    data: &'a serde_json::Value,
    plan: Plan<'a>,
}

/// Read the View's Vut-generated plan and validate it against the callback-free
/// SSR projection. The site ID is compile-time metadata; occurrences are
/// assigned after the request-specific tree has been flattened in Vut.
pub(super) fn read_plan(
    view: &Value<Nominal<ViewRenderType>>,
) -> Result<Vec<PlanEntry>, RenderError> {
    let nodes = view.get_field::<List<Nominal<ViewRenderNodeType>>>("_nodes")?;
    let entries = view.get_field::<List<Nominal<ViewHydrationPlanEntryType>>>("_hydration_plan")?;
    if nodes.len()? != entries.len()? {
        return Err(invalid(
            "View hydration plan length does not match its node list",
        ));
    }

    let mut occurrences = HashMap::<u64, u64>::new();
    let mut plan = Vec::with_capacity(entries.len()?);
    for index in 0..entries.len()? {
        let entry = entries.get(index)?;
        let node = nodes.get(index)?;
        let site_id = entry.get_field::<U64>("_site_id")?.get()?;
        let occurrence = entry.get_field::<Int>("_occurrence")?.get()?;
        let parent = entry.get_field::<Int>("_parent")?.get()?;
        let kind = read_str(&entry, "_kind")?;
        let name = read_str(&entry, "_name")?;
        let node_site_id = node.get_field::<U64>("_call_site_id")?.get()?;
        let node_parent = node.get_field::<Int>("_parent")?.get()?;
        let is_text = node.get_field::<Bool>("_is_text")?.get()?;
        let node_name = read_str(&node, "_name")?;

        if site_id == 0 || site_id != node_site_id || parent != node_parent {
            return Err(invalid(
                "View hydration plan identity does not match its node",
            ));
        }
        if occurrence < 0 {
            return Err(invalid("View hydration plan has a negative occurrence"));
        }
        let expected_kind = if is_text { "text" } else { "element" };
        if kind != expected_kind || name != node_name {
            return Err(invalid("View hydration plan shape does not match its node"));
        }

        let expected_occurrence = occurrences.entry(site_id).or_default();
        if u64::try_from(occurrence).ok() != Some(*expected_occurrence) {
            return Err(invalid(
                "View hydration plan occurrence sequence is invalid",
            ));
        }
        *expected_occurrence += 1;
        plan.push(PlanEntry {
            site: format!("v1-{site_id:016x}"),
            occurrence: *expected_occurrence - 1,
            parent,
            kind,
            name,
            site_id,
        });
    }
    Ok(plan)
}

pub(super) fn bootstrap_node(
    view: &Value<Nominal<ViewRenderType>>,
    plan: &[PlanEntry],
) -> Result<serialize::Node, RenderError> {
    let has_data = view.get_field::<Bool>("_bootstrap_has_data")?.get()?;
    let data_json = view
        .get_field::<Str>("_bootstrap_json")?
        .as_str()?
        .to_owned();
    if data_json.len() > MAX_BOOTSTRAP_BYTES {
        return Err(invalid("Hydration bootstrap data exceeds the 1 MiB limit"));
    }
    let data = serde_json::from_str::<serde_json::Value>(&data_json)
        .map_err(|_| invalid("Hydration bootstrap data is not valid JSON"))?;
    if !has_data && !data.is_null() {
        return Err(invalid(
            "No-data hydration bootstrap must contain JSON null",
        ));
    }

    let page_id = view.get_field::<U64>("_bootstrap_page_id")?.get()?;
    let envelope = BootstrapEnvelope {
        protocol: BOOTSTRAP_PROTOCOL,
        version: BOOTSTRAP_VERSION,
        page_id: format!("v1-{page_id:016x}"),
        has_data,
        data: &data,
        plan: Plan {
            version: PLAN_VERSION,
            nodes: plan,
        },
    };
    let serialized = serde_json::to_string(&envelope)
        .map_err(|_| invalid("Hydration bootstrap serialization failed"))?;
    let script_safe = script_safe_json(&serialized);
    if script_safe.len() > MAX_BOOTSTRAP_BYTES {
        return Err(invalid(
            "Hydration bootstrap payload exceeds the 1 MiB limit",
        ));
    }

    Ok(serialize::Node::Element(serialize::Element {
        name: serialize::element_name("script"),
        attributes: vec![
            (
                serialize::attribute_name("type"),
                "application/json".to_owned(),
            ),
            (
                serialize::attribute_name("data-vutnet-bootstrap"),
                "v2".to_owned(),
            ),
        ],
        children: vec![serialize::Node::Text(script_safe)],
    }))
}

pub(super) fn client_loader_node() -> serialize::Node {
    let source = crate::browser_client::asset_version().map_or_else(
        || format!("{}/bootstrap.js", crate::browser_client::ASSET_MOUNT_PREFIX),
        |version| {
            format!(
                "{}/bootstrap.js?v={version}",
                crate::browser_client::ASSET_MOUNT_PREFIX
            )
        },
    );
    serialize::Node::Element(serialize::Element {
        name: serialize::element_name("script"),
        attributes: vec![
            (serialize::attribute_name("type"), "module".to_owned()),
            (serialize::attribute_name("src"), source),
            (
                serialize::attribute_name("data-vutnet-client"),
                "v1".to_owned(),
            ),
        ],
        children: Vec::new(),
    })
}

fn script_safe_json(serialized: &str) -> String {
    serialized
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

pub(super) fn append_to_body_or_root(roots: &mut Vec<serialize::Node>, script: serialize::Node) {
    let mut script = Some(script);
    if !append_to_body(roots, &mut script)
        && let Some(script) = script
    {
        roots.push(script);
    }
}

fn append_to_body(nodes: &mut [serialize::Node], script: &mut Option<serialize::Node>) -> bool {
    for node in nodes {
        let serialize::Node::Element(element) = node else {
            continue;
        };
        if element.name.local.as_ref() == "body" {
            if let Some(script) = script.take() {
                element.children.push(script);
                return true;
            }
            return false;
        }
        if append_to_body(&mut element.children, script) {
            return true;
        }
    }
    false
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

#[cfg(test)]
mod tests {
    use super::{PlanEntry, append_to_body, script_safe_json};
    use crate::view::serialize::{self, Element, Node};

    fn entry(site_id: u64, occurrence: u64) -> PlanEntry {
        PlanEntry {
            site: format!("v1-{site_id:016x}"),
            occurrence,
            parent: -1,
            kind: "element".to_owned(),
            name: "div".to_owned(),
            site_id,
        }
    }

    #[test]
    fn marker_includes_static_site_and_runtime_occurrence() {
        assert_eq!(entry(0x1234, 7).marker(), "v2-0000000000001234-7");
    }

    #[test]
    fn bootstrap_json_escapes_script_breakout_characters() {
        let plan = [entry(0x1234, 0)];
        // Build the envelope through the same serializer used by the import.
        let data = serde_json::json!({"text": "</script><script>alert(1)</script>&"});
        let envelope = super::BootstrapEnvelope {
            protocol: super::BOOTSTRAP_PROTOCOL,
            version: super::BOOTSTRAP_VERSION,
            page_id: "v1-0000000000001234".to_owned(),
            has_data: true,
            data: &data,
            plan: super::Plan {
                version: super::PLAN_VERSION,
                nodes: &plan,
            },
        };
        let output = script_safe_json(&serde_json::to_string(&envelope).unwrap());
        assert!(!output.contains("</script>"));
        assert!(output.contains("\\u003c/script\\u003e"));
        assert!(output.contains("\\u0026"));
    }

    #[test]
    fn bootstrap_script_is_attached_to_body_when_present() {
        let script = Node::Element(Element {
            name: serialize::element_name("script"),
            attributes: Vec::new(),
            children: Vec::new(),
        });
        let mut roots = vec![Node::Element(Element {
            name: serialize::element_name("html"),
            attributes: Vec::new(),
            children: vec![Node::Element(Element {
                name: serialize::element_name("body"),
                attributes: Vec::new(),
                children: Vec::new(),
            })],
        })];
        let mut owned = Some(script);
        assert!(append_to_body(&mut roots, &mut owned));
        let Node::Element(html) = &roots[0] else {
            panic!("HTML root expected");
        };
        let Node::Element(body) = &html.children[0] else {
            panic!("body expected");
        };
        assert!(matches!(body.children.as_slice(), [Node::Element(_)]));
    }
}
