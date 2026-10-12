//! Server-side rendering for Vutnet's owned Vut View tree.

mod css;
#[allow(unsafe_code)]
mod events;
mod hydration;
mod parse;
mod raw_text;
mod schema;
mod serialize;
mod tracking;
mod validate;

use vut_native_sdk::{AbiError, Nominal, ResultValue, Runtime, Str};

use crate::identity::ViewRenderType;

#[derive(Debug)]
enum RenderError {
    Abi(AbiError),
    Invalid(String),
}

impl From<AbiError> for RenderError {
    fn from(error: AbiError) -> Self {
        Self::Abi(error)
    }
}

vut_native_sdk::managed_import!(
    pub fn _view_render_native(value: Nominal<ViewRenderType>) -> ResultValue<Str, Str> {
        let runtime = Runtime::current()?;
        let tree = value.to_owned()?;
        match render_tree(&tree) {
            Ok(html) => runtime.result_ok::<Str, Str>(runtime.copy_str(&html)?),
            Err(RenderError::Abi(error)) => Err(error),
            Err(RenderError::Invalid(message)) => {
                runtime.result_err::<Str, Str>(runtime.copy_str(&message)?)
            }
        }
    }
);

fn render_tree(
    view: &vut_native_sdk::Value<Nominal<ViewRenderType>>,
) -> Result<String, RenderError> {
    let include_markers = view
        .get_field::<vut_native_sdk::Bool>("_include_call_site_markers")?
        .get()?;
    let include_bootstrap = view
        .get_field::<vut_native_sdk::Bool>("_include_bootstrap")?
        .get()?;
    let plan = if include_markers || include_bootstrap {
        Some(hydration::read_plan(view)?)
    } else {
        None
    };
    let mut roots = parse::from_vut(view, plan.as_deref())?;
    raw_text::validate(&roots)?;
    if include_bootstrap {
        let plan = plan.as_deref().ok_or_else(|| {
            RenderError::Invalid("hydration bootstrap is missing its plan".to_owned())
        })?;
        let script = hydration::bootstrap_node(view, plan)?;
        hydration::append_to_body_or_root(&mut roots, script);
        if crate::browser_client::enabled() {
            hydration::append_to_body_or_root(&mut roots, hydration::client_loader_node());
        }
        raw_text::validate(&roots)?;
    }
    serialize::html(&roots).map_err(RenderError::Invalid)
}
