use std::convert::Infallible;

use futures_util::stream;
use http::{
    HeaderMap,
    header::{CONTENT_DISPOSITION, CONTENT_TYPE},
};
use multer::bytes::Bytes;
use multer::{Constraints, Multipart, SizeLimit};
use server_native::ServerRequest;

use super::{Form, Issue, Upload};

const MAX_PARTS: usize = 1024;

pub(crate) async fn parse(request: &ServerRequest) -> Form {
    match parse_form(request.headers(), request.body().clone()).await {
        Ok(form) => form,
        Err(issue) => Form::failed(issue),
    }
}

async fn parse_form(headers: &HeaderMap, body: Bytes) -> Result<Form, Issue> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .ok_or(Issue::NotMultipart)?
        .to_str()
        .map_err(|_| Issue::Malformed)?;
    let media: mime::Mime = content_type.parse().map_err(|_| Issue::Malformed)?;
    if media.type_() != mime::MULTIPART || media.subtype() != mime::FORM_DATA {
        return Err(Issue::NotMultipart);
    }
    if media.get_param(mime::BOUNDARY).is_none() {
        return Err(Issue::MissingBoundary);
    }
    let boundary = multer::parse_boundary(content_type).map_err(|_| Issue::InvalidBoundary)?;
    // RFC 2046 bchars: 1..70 ASCII bytes, without a trailing space.
    if !valid_boundary(&boundary) {
        return Err(Issue::InvalidBoundary);
    }
    let bound = body.len() as u64;
    let source = stream::once(async move { Ok::<_, Infallible>(body) });
    // The server has already enforced the configured bound before this parser.
    // Repeat a finite bound here so field extraction cannot bypass it.
    let constraints =
        Constraints::new().size_limit(SizeLimit::new().whole_stream(bound).per_field(bound));
    let mut parser = Multipart::with_constraints(source, boundary, constraints);
    let mut form = Form::default();
    while let Some(field) = parser.next_field().await.map_err(|_| Issue::Malformed)? {
        if form.fields.len() + form.files.len() >= MAX_PARTS {
            return Err(Issue::TooManyParts);
        }
        let disposition = field
            .headers()
            .get(CONTENT_DISPOSITION)
            .ok_or(Issue::InvalidDisposition)?
            .to_str()
            .map_err(|_| Issue::InvalidDisposition)?;
        if !disposition
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("form-data")
        {
            return Err(Issue::InvalidDisposition);
        }
        let name = field.name().ok_or(Issue::InvalidDisposition)?.to_owned();
        let filename = field.file_name().map(str::to_owned);
        let content_type = match field.headers().get(CONTENT_TYPE) {
            Some(value) => {
                let value = value.to_str().map_err(|_| Issue::Malformed)?;
                let mime: mime::Mime = value.parse().map_err(|_| Issue::Malformed)?;
                Some(mime.to_string())
            }
            None => None,
        };
        let bytes = field.bytes().await.map_err(|_| Issue::Malformed)?;
        if filename.is_some() {
            form.files.push(Upload {
                name,
                filename,
                content_type,
                bytes,
            });
        } else {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| Issue::InvalidText)?
                .to_owned();
            form.fields.push((name, text));
        }
    }
    Ok(form)
}

fn valid_boundary(boundary: &str) -> bool {
    !boundary.is_empty()
        && boundary.len() <= 70
        && !boundary.ends_with(' ')
        && boundary
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"'()+_,-./:=? ".contains(&byte))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
