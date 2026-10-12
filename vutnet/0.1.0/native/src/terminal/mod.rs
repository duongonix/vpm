//! Structured terminal output for the Vutnet native server.

mod format;

use std::{
    io::Write as _,
    net::SocketAddr,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use http::StatusCode;

static OUTPUT_LOCK: Mutex<()> = Mutex::new(());

pub(crate) const DEV_SUPERVISOR_ENV: &str = "VUTNET_CLI_SUPERVISED";
pub(crate) const DEV_STARTUP_DETAILS_ENV: &str = "VUTNET_CLI_STARTUP_DETAILS";
pub(crate) const DEV_REQUESTS_VERBOSE_ENV: &str = "VUTNET_DEV_VERBOSE";

pub(crate) fn dev_verbose_requests() -> bool {
    std::env::var_os(DEV_SUPERVISOR_ENV).is_some()
        && std::env::var(DEV_REQUESTS_VERBOSE_ENV).as_deref() == Ok("1")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestSource {
    Application,
    StaticFiles,
}

pub(crate) enum Event {
    Ready {
        version: &'static str,
        address: SocketAddr,
        elapsed: Duration,
        mode: Option<String>,
        watching: Option<String>,
    },
    DevReady {
        address: SocketAddr,
        elapsed: Duration,
        mode: Option<String>,
        watching: Option<String>,
    },
    Request {
        method: String,
        path: String,
        status: StatusCode,
        elapsed: Duration,
    },
    RequestError {
        method: String,
        path: String,
        status: StatusCode,
        elapsed: Duration,
        stage: &'static str,
        message: String,
    },
    Error {
        message: String,
    },
    Stopped,
}

pub(crate) fn emit(event: Event) {
    let _guard = output_lock();
    let rendered = format::render(event, true);
    match rendered.stream {
        format::Stream::Stdout => {
            let mut output = anstream::stdout();
            let _ = output.write_all(rendered.text.as_bytes());
            let _ = output.flush();
        }
        format::Stream::Stderr => {
            let mut output = anstream::stderr();
            let _ = output.write_all(rendered.text.as_bytes());
            let _ = output.flush();
        }
    }
}

fn output_lock() -> MutexGuard<'static, ()> {
    OUTPUT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn error(message: impl Into<String>) {
    emit(Event::Error {
        message: message.into(),
    });
}

pub(crate) fn server_ready(address: SocketAddr, elapsed: Duration) {
    let mode = std::env::var("VUTNET_CLI_MODE").ok();
    let watching = std::env::var("VUTNET_CLI_WATCHING").ok();
    if std::env::var_os(DEV_SUPERVISOR_ENV).is_some() {
        let show_startup_details = std::env::var(DEV_STARTUP_DETAILS_ENV).as_deref() == Ok("1");
        if show_startup_details {
            emit(Event::DevReady {
                address,
                elapsed,
                mode,
                watching,
            });
        }
        return;
    }

    emit(Event::Ready {
        version: env!("CARGO_PKG_VERSION"),
        address,
        elapsed,
        mode,
        watching,
    });
}

pub(crate) fn request(
    method: &http::Method,
    path: &str,
    status: StatusCode,
    elapsed: Duration,
    source: RequestSource,
    verbose: bool,
) {
    if !should_log_request(source, status, verbose) {
        return;
    }
    emit(Event::Request {
        method: method.as_str().to_owned(),
        path: path.to_owned(),
        status,
        elapsed,
    });
}

fn should_log_request(source: RequestSource, status: StatusCode, verbose: bool) -> bool {
    verbose
        || source == RequestSource::Application
        || !(status.is_success() || status.is_redirection())
}

pub(crate) fn request_error(
    method: &http::Method,
    path: &str,
    status: StatusCode,
    elapsed: Duration,
    stage: &'static str,
    message: impl Into<String>,
) {
    emit(Event::RequestError {
        method: method.as_str().to_owned(),
        path: path.to_owned(),
        status,
        elapsed,
        stage,
        message: message.into(),
    });
}

pub(crate) fn server_stopped() {
    if std::env::var_os(DEV_SUPERVISOR_ENV).is_none() {
        emit(Event::Stopped);
    }
}

#[cfg(test)]
mod request_tests;
