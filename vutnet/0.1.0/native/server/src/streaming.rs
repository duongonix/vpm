//! Move-only native file transport; public Vut responses remain buffered.

use std::{fs::File, io};

use bytes::Bytes;
use futures_util::TryStreamExt;
use http::{
    HeaderValue, Response,
    header::{CONTENT_LENGTH, CONTENT_TYPE},
};
use http_body_util::{BodyExt, StreamBody, combinators::UnsyncBoxBody};
use hyper::body::Frame;
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

use crate::ServerResponse;

pub(crate) type HttpBody = UnsyncBoxBody<Bytes, io::Error>;

/// Native transport extension. Each response owns and consumes its payload.
#[derive(Debug)]
pub enum NativeResponse {
    /// The existing cloneable, buffered server response.
    Buffered(ServerResponse),
    /// An open regular file streamed with bounded chunks.
    File(FileResponse),
}

impl NativeResponse {
    /// The HTTP status that will be sent to the client.
    #[must_use]
    pub fn status(&self) -> http::StatusCode {
        match self {
            Self::Buffered(response) => response.status(),
            Self::File(_) => http::StatusCode::OK,
        }
    }

    pub(crate) fn into_http(self) -> Response<HttpBody> {
        match self {
            Self::Buffered(response) => response.into_http(),
            Self::File(response) => response.into_http(),
        }
    }
}

/// Owns an already validated/opened file. Dropping it closes the descriptor,
/// including when a client disconnects before the stream has completed.
#[derive(Debug)]
pub struct FileResponse {
    file: File,
    length: u64,
    content_type: HeaderValue,
}

impl FileResponse {
    /// Streams at most `length` bytes from this independently opened file.
    #[must_use]
    pub fn new(file: File, length: u64, content_type: HeaderValue) -> Self {
        Self {
            file,
            length,
            content_type,
        }
    }

    fn into_http(self) -> Response<HttpBody> {
        let reader = tokio::fs::File::from_std(self.file).take(self.length);
        let chunks = ReaderStream::with_capacity(reader, 64 * 1024).map_ok(Frame::data);
        let body = StreamBody::new(chunks).boxed_unsync();
        let mut response = Response::new(body);
        response
            .headers_mut()
            .insert(CONTENT_TYPE, self.content_type);
        response
            .headers_mut()
            .insert(CONTENT_LENGTH, HeaderValue::from(self.length));
        response
    }
}
