#![allow(unsafe_code)]

use vut_native_abi::VutValueHandleV1;
use vut_native_sdk::{CallbackSignature, List, Nominal, NominalType, Value};

include!(concat!(env!("OUT_DIR"), "/type_ids.rs"));

pub(crate) enum ContextType {}
pub(crate) enum DispatchPipelineType {}
pub(crate) enum ResponseType {}
pub(crate) enum HeaderType {}
pub(crate) enum RouteEntryType {}
pub(crate) enum KeyValueType {}
pub(crate) enum StaticMountType {}
pub(crate) enum MultipartType {}
pub(crate) enum MultipartFieldType {}
pub(crate) enum MultipartFileType {}
pub(crate) enum ViewAttributeType {}
pub(crate) enum ViewHydrationPlanEntryType {}
pub(crate) enum ViewRenderType {}
pub(crate) enum ViewRenderNodeType {}

macro_rules! nominal_type {
    ($marker:ty, $type_id:ident, $schema_version:expr) => {
        // SAFETY: build.rs derives this from the canonical VPM identity and
        // fully qualified Vut type name for the schema this crate consumes.
        unsafe impl NominalType for $marker {
            const TYPE_ID: VutTypeIdV1 = $type_id;
            const SCHEMA_VERSION: u32 = $schema_version;
        }
    };
}

nominal_type!(ContextType, CONTEXT, CONTEXT_SCHEMA_VERSION);
nominal_type!(
    DispatchPipelineType,
    DISPATCH_PIPELINE,
    DISPATCH_PIPELINE_SCHEMA_VERSION
);
nominal_type!(ResponseType, RESPONSE, RESPONSE_SCHEMA_VERSION);
nominal_type!(HeaderType, HEADER, HEADER_SCHEMA_VERSION);
nominal_type!(RouteEntryType, ROUTE_ENTRY, ROUTE_ENTRY_SCHEMA_VERSION);
nominal_type!(KeyValueType, KEY_VALUE, KEY_VALUE_SCHEMA_VERSION);
nominal_type!(StaticMountType, STATIC_MOUNT, STATIC_MOUNT_SCHEMA_VERSION);
nominal_type!(MultipartType, MULTIPART, MULTIPART_SCHEMA_VERSION);
nominal_type!(
    MultipartFieldType,
    MULTIPART_FIELD,
    MULTIPART_FIELD_SCHEMA_VERSION
);
nominal_type!(
    MultipartFileType,
    MULTIPART_FILE,
    MULTIPART_FILE_SCHEMA_VERSION
);
nominal_type!(
    ViewAttributeType,
    VIEW_ATTRIBUTE,
    VIEW_ATTRIBUTE_SCHEMA_VERSION
);
nominal_type!(
    ViewHydrationPlanEntryType,
    VIEW_HYDRATION_PLAN_ENTRY,
    VIEW_HYDRATION_PLAN_ENTRY_SCHEMA_VERSION
);
nominal_type!(ViewRenderType, VIEW_RENDER, VIEW_RENDER_SCHEMA_VERSION);
nominal_type!(
    ViewRenderNodeType,
    VIEW_RENDER_NODE,
    VIEW_RENDER_NODE_SCHEMA_VERSION
);

pub(crate) struct Controller;
impl CallbackSignature for Controller {
    type Arguments = (Value<Nominal<ContextType>>,);
    type Return = Nominal<ResponseType>;
    const SIGNATURE_ID: VutTypeIdV1 = CONTROLLER_SIGNATURE;
}

pub(crate) type ControllerHandle = vut_native_sdk::Callback<Controller>;

pub(crate) fn borrowed_list<T: NominalType>(
    runtime: &vut_native_sdk::Runtime,
    raw: u64,
) -> Result<Value<List<Nominal<T>>>, vut_native_sdk::AbiError> {
    // SAFETY: this function is called synchronously from the declared native
    // import while the compiler-owned list value remains live.
    unsafe { Value::from_borrowed_raw(runtime, VutValueHandleV1 { token: raw }) }
}
