//! Minimal real-socket HTTP/1.1 client used by the integration tests.
//!
//! The tests intentionally use raw TCP rather than an HTTP client crate so the
//! server is exercised against the real wire protocol and malformed framing.
#![allow(dead_code)]

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn connect(address: SocketAddr) -> TcpStream {
    TcpStream::connect(address)
        .await
        .expect("connect to server")
}

/// Sends raw bytes, closes the write side via `Connection: close`, and reads
/// until the peer closes.
pub async fn send_raw(address: SocketAddr, request: &[u8]) -> Vec<u8> {
    let mut stream = connect(address).await;
    stream.write_all(request).await.expect("write request");
    stream.flush().await.expect("flush request");
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.expect("read response");
    bytes
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub fn get_request(path: &str) -> Vec<u8> {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").into_bytes()
}

pub fn get_request_keep_alive(path: &str) -> Vec<u8> {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes()
}

/// Parses a complete buffered HTTP/1.1 response.
pub fn parse_response(raw: &[u8]) -> Response {
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response must contain a header terminator");
    let header_end = split + 4;
    let header_text = String::from_utf8_lossy(&raw[..header_end]).into_owned();
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .expect("status code");
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = raw[header_end..].to_vec();
    body.truncate(length);
    Response {
        status,
        headers,
        body,
    }
}

/// Reads exactly one framed HTTP/1.1 response from a live connection.
pub async fn read_response(stream: &mut TcpStream) -> Response {
    let mut buffer = Vec::new();
    let header_end = loop {
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
        let mut chunk = [0_u8; 1024];
        let read = stream
            .read(&mut chunk)
            .await
            .expect("read response headers");
        assert!(
            read > 0,
            "connection closed before response headers: {:?}",
            String::from_utf8_lossy(&buffer)
        );
        buffer.extend_from_slice(&chunk[..read]);
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .expect("status code");
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = buffer[header_end..].to_vec();
    while body.len() < length {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).await.expect("read response body");
        assert!(read > 0, "connection closed before response body");
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);

    Response {
        status,
        headers,
        body,
    }
}
