use vut_native_abi::{
    AbiSchemaManifestError, VutTypeIdV1, VutTypeSchemaV1, nominal_type_id_v1,
    schema_version_from_manifest_v1, signature_id_v1,
};

// Defaults mirror each declaration's @abi_schema(N), or N=1 when there is no
// explicit annotation. A CLI-only `[[bin]]` legitimately omits framework
// nominals from its compiler manifest. Framework apps use emitted versions
// whenever their managed native imports make the types reachable.
const NOMINALS: &[(&str, &str, u32)] = &[
    ("CONTEXT", "vutnet.context::Context", 1),
    ("DISPATCH_PIPELINE", "vutnet.context::DispatchPipeline", 1),
    ("RESPONSE", "vutnet.response::Response", 2),
    ("HEADER", "vutnet.response::Header", 1),
    ("ROUTE_ENTRY", "vutnet.router::RouteEntry", 1),
    ("KEY_VALUE", "vutnet.context::KeyValue", 1),
    ("STATIC_MOUNT", "vutnet.static_files::StaticMount", 1),
    ("MULTIPART", "vutnet.multipart::Multipart", 1),
    ("MULTIPART_FIELD", "vutnet.multipart::MultipartField", 1),
    ("MULTIPART_FILE", "vutnet.multipart::MultipartFile", 1),
    ("VIEW_ATTRIBUTE", "vutnet.view::Attribute", 1),
    (
        "VIEW_HYDRATION_PLAN_ENTRY",
        "vutnet.view.hydration::HydrationPlanEntry",
        1,
    ),
    ("VIEW_RENDER", "vutnet.view::ViewRender", 3),
    ("VIEW_RENDER_NODE", "vutnet.view::ViewRenderNode", 2),
];

pub struct NominalIdentity {
    pub constant: &'static str,
    pub type_id: VutTypeIdV1,
    pub schema_version: u32,
}

pub struct VutnetAbi {
    pub nominals: Vec<NominalIdentity>,
    pub controller_signature: VutTypeIdV1,
}

impl VutnetAbi {
    pub fn from_manifest(package: &str, manifest: &str) -> Self {
        let nominals = NOMINALS
            .iter()
            .map(|(constant, qualified, default_schema_version)| {
                let type_id = nominal_type_id_v1(package, qualified)
                    .unwrap_or_else(|error| panic!("invalid Vutnet type identity: {error:?}"));
                let schema_version = match schema_version_from_manifest_v1(manifest, type_id) {
                    Ok(version) => version,
                    Err(AbiSchemaManifestError::MissingType) => *default_schema_version,
                    Err(error) => panic!("invalid Vutnet schema for {qualified}: {error}"),
                };
                NominalIdentity {
                    constant,
                    type_id,
                    schema_version,
                }
            })
            .collect::<Vec<_>>();
        let context = schema(&nominals, "CONTEXT");
        let response = schema(&nominals, "RESPONSE");
        let controller_signature =
            signature_id_v1(None, &[context], response).expect("Controller signature identity");
        Self {
            nominals,
            controller_signature,
        }
    }
}

fn schema(nominals: &[NominalIdentity], constant: &str) -> VutTypeSchemaV1 {
    let nominal = nominals
        .iter()
        .find(|nominal| nominal.constant == constant)
        .expect("required Vutnet nominal identity");
    VutTypeSchemaV1 {
        type_id: nominal.type_id,
        schema_version: nominal.schema_version,
    }
}
