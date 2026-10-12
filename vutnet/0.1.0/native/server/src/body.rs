use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::Incoming;

/// Why a bounded body read stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyError {
    /// The peer sent more than the configured limit.
    TooLarge,
    /// The body stream failed (reset, malformed framing, incomplete body).
    Read,
}

/// Reads a request body into memory while enforcing a hard byte limit.
///
/// The limit is checked before every append, so the buffer never exceeds
/// `limit` even when the peer keeps streaming. Frame trailers are ignored.
pub(crate) async fn collect_bounded(body: Incoming, limit: usize) -> Result<Bytes, BodyError> {
    let mut collected = BytesMut::new();
    let mut body = body;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| BodyError::Read)?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        if collected.len().saturating_add(data.len()) > limit {
            return Err(BodyError::TooLarge);
        }
        collected.extend_from_slice(&data);
    }
    Ok(collected.freeze())
}
