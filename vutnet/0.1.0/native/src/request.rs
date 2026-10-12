use std::collections::BTreeMap;

use http::Method;
use percent_encoding::percent_decode_str;
use server_native::ServerRequest;

use vut_native_sdk::{Nominal, Value};

use crate::identity::{ControllerHandle, DispatchPipelineType};

pub(crate) struct Endpoint {
    pub(crate) controller: ControllerHandle,
    pub(crate) pipeline: Value<Nominal<DispatchPipelineType>>,
}

pub(crate) struct PathRoute {
    pub(crate) methods: BTreeMap<Method, std::sync::Arc<Endpoint>>,
}

pub(crate) struct RequestParts {
    pub(crate) params: Vec<(String, String)>,
    pub(crate) headers: Vec<(String, String)>,
}

pub(crate) fn build_parts(
    request: &ServerRequest,
    matched: &matchit::Match<'_, '_, &std::sync::Arc<PathRoute>>,
) -> Result<RequestParts, ()> {
    let mut params = Vec::with_capacity(matched.params.len());
    for (name, value) in matched.params.iter() {
        let decoded = decode_component(value)?;
        params.push((name.to_owned(), decoded));
    }

    let mut headers = Vec::new();
    for (name, value) in request.headers() {
        let text = value.to_str().map_err(|_| ())?;
        headers.push((name.as_str().to_ascii_lowercase(), text.to_owned()));
    }

    Ok(RequestParts { params, headers })
}

pub(crate) fn decode_query(query: Option<&str>) -> Result<Vec<(String, String)>, ()> {
    let Some(query) = query else {
        return Ok(Vec::new());
    };
    let mut decoded = Vec::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        decoded.push((decode_form_component(key)?, decode_form_component(value)?));
    }
    Ok(decoded)
}

fn decode_form_component(value: &str) -> Result<String, ()> {
    if !valid_percent_escapes(value) {
        return Err(());
    }
    let space_encoded = value.replace('+', " ");
    percent_decode_str(&space_encoded)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| ())
}

pub(crate) fn decode_component(value: &str) -> Result<String, ()> {
    if !valid_percent_escapes(value) {
        return Err(());
    }
    percent_decode_str(value)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| ())
}

fn valid_percent_escapes(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

pub(crate) fn normalized_request_path(path: &str) -> String {
    if path == "/" {
        return "/".to_owned();
    }
    path.trim_end_matches('/').to_owned()
}

pub(crate) fn matchit_pattern(path: &str) -> String {
    let mut output = String::new();
    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        output.push('/');
        if let Some(name) = segment.strip_prefix(':') {
            output.push('{');
            output.push_str(name);
            output.push('}');
        } else {
            output.push_str(segment);
        }
    }
    if output.is_empty() {
        "/".to_owned()
    } else {
        output
    }
}

pub(crate) fn method_from_name(name: &str) -> Result<Method, ()> {
    Method::from_bytes(name.as_bytes()).map_err(|_| ())
}
