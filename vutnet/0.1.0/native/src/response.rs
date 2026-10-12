use http::{HeaderName, HeaderValue, StatusCode, header::CONTENT_TYPE};
use server_native::ServerResponse;
use vut_native_sdk::{Bytes, Int, List, Nominal, Str, Value};

use crate::identity::{HeaderType, ResponseType};

pub(crate) struct MappedResponse {
    pub(crate) response: ServerResponse,
    pub(crate) diagnostic: Option<String>,
}

pub(crate) fn from_vut(
    value: &Value<Nominal<ResponseType>>,
) -> Result<MappedResponse, &'static str> {
    let status = read_status(value)?;
    let body = read_body(value)?;
    let content_type = read_content_type(value)?;
    let diagnostic = read_diagnostic(value)?;
    let headers = read_headers(value)?;
    let response = headers.into_iter().fold(
        ServerResponse::bytes(status, body).with_header(CONTENT_TYPE, content_type),
        |response, (name, value)| response.with_header(name, value),
    );
    Ok(MappedResponse {
        response,
        diagnostic,
    })
}

fn read_status(value: &Value<Nominal<ResponseType>>) -> Result<StatusCode, &'static str> {
    let status = value
        .get_field::<Int>("_status")
        .map_err(|_| "could not read Response._status")?
        .get()
        .map_err(|_| "Response._status is not an integer")?;
    let status = u16::try_from(status).map_err(|_| "Response status is outside HTTP range")?;
    let status = StatusCode::from_u16(status).map_err(|_| "Response status is invalid")?;
    if status.is_informational() || status.as_u16() > 599 {
        return Err("informational and out-of-range statuses are not supported");
    }
    Ok(status)
}

fn read_body(value: &Value<Nominal<ResponseType>>) -> Result<Vec<u8>, &'static str> {
    let body = value
        .get_field::<Bytes>("_body")
        .map_err(|_| "could not read Response._body")?;
    Ok(body
        .as_bytes()
        .map_err(|_| "could not read Response body bytes")?
        .to_vec())
}

fn read_content_type(value: &Value<Nominal<ResponseType>>) -> Result<HeaderValue, &'static str> {
    let content_type = value
        .get_field::<Str>("_content_type")
        .map_err(|_| "could not read Response._content_type")?;
    HeaderValue::from_bytes(
        content_type
            .as_str()
            .map_err(|_| "Response._content_type is not valid UTF-8")?
            .as_bytes(),
    )
    .map_err(|_| "Response content type is not a valid HTTP header value")
}

fn read_diagnostic(value: &Value<Nominal<ResponseType>>) -> Result<Option<String>, &'static str> {
    let diagnostic = value
        .get_field::<Str>("_diagnostic")
        .map_err(|_| "could not read Response._diagnostic")?;
    let diagnostic = diagnostic
        .as_str()
        .map_err(|_| "Response._diagnostic is not valid UTF-8")?;
    Ok((!diagnostic.is_empty()).then(|| diagnostic.to_owned()))
}

fn read_headers(
    value: &Value<Nominal<ResponseType>>,
) -> Result<Vec<(HeaderName, HeaderValue)>, &'static str> {
    let headers = value
        .get_field::<List<Nominal<HeaderType>>>("_headers")
        .map_err(|_| "could not read Response._headers")?;
    let mut output = Vec::with_capacity(
        headers
            .len()
            .map_err(|_| "could not read Response header list length")?,
    );
    for index in 0..headers
        .len()
        .map_err(|_| "could not read Response header list length")?
    {
        let header = headers
            .get(index)
            .map_err(|_| "could not read Response header")?;
        output.push(read_header(&header)?);
    }
    Ok(output)
}

fn read_header(
    header: &Value<Nominal<HeaderType>>,
) -> Result<(HeaderName, HeaderValue), &'static str> {
    let name = header
        .get_field::<Str>("_name")
        .map_err(|_| "could not read Response header name")?;
    let name = name
        .as_str()
        .map_err(|_| "Response header name is not valid UTF-8")?;
    let value = header
        .get_field::<Str>("_value")
        .map_err(|_| "could not read Response header value")?;
    let value = value
        .as_str()
        .map_err(|_| "Response header value is not valid UTF-8")?;
    let name =
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| "Response header name is invalid")?;
    let value = HeaderValue::from_bytes(value.as_bytes())
        .map_err(|_| "Response header value is invalid")?;
    Ok((name, value))
}
