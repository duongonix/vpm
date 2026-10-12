mod support;

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use http::StatusCode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vutnet_server_native::{
    FixedHandler, Handler, Server, ServerConfig, ServerError, ServerRequest, ServerResponse,
    ShutdownHandle,
};

fn loopback() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
}

fn config() -> ServerConfig {
    ServerConfig::bind(loopback())
}

struct Running {
    address: SocketAddr,
    trigger: ShutdownHandle,
    task: tokio::task::JoinHandle<Result<(), ServerError>>,
}

impl Running {
    async fn stop(self) {
        self.trigger.shutdown();
        let result = self.task.await.expect("server task must not panic");
        result.expect("server must shut down cleanly");
    }
}

async fn start_with<H: Handler>(config: ServerConfig, handler: H) -> Running {
    let server = Server::bind(config).await.expect("bind server");
    let address = server.local_addr().expect("local address");
    let shutdown = ShutdownHandle::new();
    let trigger = shutdown.clone();
    let task = tokio::spawn(server.serve_with_shutdown(handler, shutdown));
    Running {
        address,
        trigger,
        task,
    }
}

async fn start_fixed_with(config: ServerConfig, status: StatusCode, body: &'static str) -> Running {
    start_with(config, FixedHandler::text(status, body)).await
}

async fn start_fixed(status: StatusCode, body: &'static str) -> Running {
    start_fixed_with(config(), status, body).await
}

#[tokio::test]
async fn serves_a_fixed_response_over_a_real_socket() {
    let running = start_fixed(StatusCode::OK, "hello fixed").await;

    let raw = support::send_raw(running.address, &support::get_request("/any/path")).await;
    let response = support::parse_response(&raw);

    assert_eq!(response.status, 200);
    assert_eq!(response.text(), "hello fixed");
    assert_eq!(
        response.header("content-type"),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(response.header("content-length"), Some("11"));

    running.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn serves_concurrent_real_requests() {
    let running = start_fixed(StatusCode::OK, "concurrent").await;
    let address = running.address;

    let mut tasks = Vec::new();
    for index in 0..32 {
        tasks.push(tokio::spawn(async move {
            let raw = support::send_raw(address, &support::get_request(&format!("/{index}"))).await;
            support::parse_response(&raw)
        }));
    }

    for task in tasks {
        let response = task.await.expect("request task");
        assert_eq!(response.status, 200);
        assert_eq!(response.text(), "concurrent");
    }

    running.stop().await;
}

#[tokio::test]
async fn delivers_an_owned_request_representation() {
    let running = start_with(config(), |request: ServerRequest| async move {
        let body_length = request.body().len();
        let peer = request.peer();
        assert!(peer.ip().is_loopback());
        assert!(request.request_id() < u64::MAX);
        ServerResponse::text(
            StatusCode::OK,
            format!(
                "{} {} {} {}",
                request.method(),
                request.path(),
                request.query().unwrap_or("-"),
                body_length
            ),
        )
    })
    .await;

    let request = b"POST /echo?x=1 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 4\r\n\r\nbody";
    let raw = support::send_raw(running.address, request).await;
    let response = support::parse_response(&raw);

    assert_eq!(response.status, 200);
    assert_eq!(response.text(), "POST /echo x=1 4");

    running.stop().await;
}

#[tokio::test]
async fn rejects_declared_oversized_body_without_reading_it() {
    let running = start_fixed_with(config().with_max_body_bytes(8), StatusCode::OK, "small").await;

    // Declares 100 bytes but sends none: the limit must reject before reading.
    let request =
        b"POST / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 100\r\n\r\n";
    let raw = support::send_raw(running.address, request).await;
    let response = support::parse_response(&raw);

    assert_eq!(response.status, 413);
    assert_eq!(response.text(), "request body too large");

    running.stop().await;
}

#[tokio::test]
async fn rejects_streamed_oversized_body() {
    let running = start_fixed_with(config().with_max_body_bytes(8), StatusCode::OK, "small").await;

    let request = b"POST / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n\
10\r\n0123456789abcdef\r\n10\r\n0123456789abcdef\r\n0\r\n\r\n";
    let raw = support::send_raw(running.address, request).await;
    let response = support::parse_response(&raw);

    assert_eq!(response.status, 413);
    assert_eq!(response.text(), "request body too large");

    running.stop().await;
}

#[tokio::test]
async fn rejects_malformed_requests_deterministically() {
    let running = start_fixed(StatusCode::OK, "never").await;

    for request in [
        b"not a request\r\n\r\n".as_slice(),
        b"GET / HTTP/1.1\r\nHost: x\r\nMissing Colon Header\r\n\r\n".as_slice(),
    ] {
        let raw = support::send_raw(running.address, request).await;
        assert!(
            raw.is_empty() || raw.starts_with(b"HTTP/1.1 400"),
            "malformed request must not be served: {:?}",
            String::from_utf8_lossy(&raw)
        );
    }

    running.stop().await;
}

#[tokio::test]
async fn shutdown_closes_idle_keep_alive_connections() {
    let running = start_fixed(StatusCode::OK, "keep-alive").await;

    let mut stream = support::connect(running.address).await;
    stream
        .write_all(&support::get_request_keep_alive("/"))
        .await
        .expect("write request");
    let response = support::read_response(&mut stream).await;
    assert_eq!(response.status, 200);
    assert_eq!(response.text(), "keep-alive");

    running.trigger.shutdown();

    let mut buffer = [0_u8; 16];
    let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer))
        .await
        .expect("shutdown must close the idle connection")
        .expect("read after shutdown");
    assert_eq!(read, 0, "keep-alive connection must be closed on shutdown");

    let result = running.task.await.expect("server task");
    result.expect("server must shut down cleanly");

    // The listener must no longer serve after the server future resolves. A
    // closed port may fail or hang the connect depending on the OS, so assert
    // the meaningful property: no successful HTTP response is produced.
    let served = tokio::time::timeout(Duration::from_secs(2), async {
        match tokio::net::TcpStream::connect(running.address).await {
            Ok(mut stream) => {
                let _ = stream.write_all(&support::get_request("/")).await;
                let mut buffer = Vec::new();
                let _ =
                    tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut buffer))
                        .await;
                Some(buffer)
            }
            Err(_) => None,
        }
    })
    .await;
    if let Ok(Some(response)) = served {
        assert!(
            !response.starts_with(b"HTTP/1.1 200"),
            "listener must stop serving after shutdown: {:?}",
            String::from_utf8_lossy(&response)
        );
    }
}

#[tokio::test]
async fn repeated_lifecycle_does_not_leak_connections() {
    for _ in 0..8 {
        let running = start_fixed(StatusCode::OK, "cycle").await;
        let raw = support::send_raw(running.address, &support::get_request("/")).await;
        assert_eq!(support::parse_response(&raw).status, 200);
        running.stop().await;
    }
}
