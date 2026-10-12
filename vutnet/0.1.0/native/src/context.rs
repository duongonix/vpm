use std::net::SocketAddr;

use server_native::ServerRequest;
use vut_native_sdk::{Int, List, NamedValue, Nominal, Runtime, Value};

use crate::identity::{ContextType, DispatchPipelineType, KeyValueType};
use crate::request::{RequestParts, decode_query};

type PairValue = Value<Nominal<KeyValueType>>;
type PairList = Value<List<Nominal<KeyValueType>>>;
type PipelineValue = Value<Nominal<DispatchPipelineType>>;

fn build_pairs(
    runtime: &Runtime,
    pairs: &[(String, String)],
) -> Result<PairList, vut_native_sdk::AbiError> {
    let mut values: Vec<PairValue> = Vec::with_capacity(pairs.len());
    for (key, value) in pairs {
        let key_value = runtime.copy_str(key)?;
        let text_value = runtime.copy_str(value)?;
        values.push(runtime.build_data::<KeyValueType>(&[
            NamedValue::new("key", &key_value),
            NamedValue::new("value", &text_value),
        ])?);
    }
    runtime.build_list(&values)
}

pub(crate) fn build(
    runtime: &Runtime,
    request: &ServerRequest,
    parts: &RequestParts,
    peer: SocketAddr,
    pipeline: &PipelineValue,
    form: &crate::multipart::Form,
) -> Result<Value<Nominal<ContextType>>, ContextBuildError> {
    let method = runtime.copy_str(request.method().as_str())?;
    let path = runtime.copy_str(request.path())?;
    let params = build_pairs(runtime, &parts.params)?;
    let query_pairs = decode_query(request.query()).map_err(|()| ContextBuildError::BadRequest)?;
    let query = build_pairs(runtime, &query_pairs)?;
    let headers = build_pairs(runtime, &parts.headers)?;
    let body = runtime.copy_bytes(request.body().as_ref())?;
    let multipart = crate::multipart::build(runtime, form)?;
    let client_ip = runtime.copy_str(&peer.ip().to_string())?;
    let client_port = runtime.copy_primitive::<Int>(i64::from(peer.port()))?;
    let request_id =
        i64::try_from(request.request_id()).map_err(|_| ContextBuildError::Internal)?;
    let request_id = runtime.copy_primitive::<Int>(request_id)?;
    let global_index = runtime.copy_primitive::<Int>(0)?;
    let layer_index = runtime.copy_primitive::<Int>(0)?;
    let scope_index = runtime.copy_primitive::<Int>(0)?;
    let middleware_index = runtime.copy_primitive::<Int>(0)?;

    Ok(runtime.build_data::<ContextType>(&[
        NamedValue::new("_method", &method),
        NamedValue::new("_path", &path),
        NamedValue::new("_params", &params),
        NamedValue::new("_query", &query),
        NamedValue::new("_headers", &headers),
        NamedValue::new("_body", &body),
        NamedValue::new("_multipart", &multipart),
        NamedValue::new("_client_ip", &client_ip),
        NamedValue::new("_client_port", &client_port),
        NamedValue::new("_request_id", &request_id),
        NamedValue::new("_pipeline", pipeline),
        NamedValue::new("_global_index", &global_index),
        NamedValue::new("_layer_index", &layer_index),
        NamedValue::new("_scope_index", &scope_index),
        NamedValue::new("_middleware_index", &middleware_index),
    ])?)
}

#[derive(Debug)]
pub(crate) enum ContextBuildError {
    BadRequest,
    Internal,
    Abi(vut_native_sdk::AbiError),
}

impl From<vut_native_sdk::AbiError> for ContextBuildError {
    fn from(error: vut_native_sdk::AbiError) -> Self {
        Self::Abi(error)
    }
}
