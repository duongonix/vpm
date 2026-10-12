#![allow(unsafe_code)]

use std::{
    net::{Ipv4Addr, SocketAddr},
    panic::{AssertUnwindSafe, catch_unwind},
    time::Instant,
};

use server_native::{Server, ServerConfig, ShutdownHandle};
use tokio::runtime::Builder;
use vut_native_abi::{
    VUT_FEATURE_CALLABLE_FIELDS_V1, VUT_FEATURE_CALLBACK_ASYNC_V1, VUT_FEATURE_CALLBACK_SYNC_V1,
    VUT_FEATURE_DATA_FIELDS_V1, VUT_FEATURE_VALUE_LISTS_V1,
};
use vut_native_sdk::Runtime;

use crate::{
    identity,
    routing::{RouteTable, VutHandler},
    terminal,
};

const STATUS_OK: i32 = 0;
const STATUS_INVALID_PORT: i32 = -1;
const STATUS_RUNTIME: i32 = -2;
const STATUS_BIND: i32 = -3;
const STATUS_ROUTER: i32 = -4;
const STATUS_ABI: i32 = -5;
const STATUS_INVALID_BODY_LIMIT: i32 = -6;
const STATUS_STATIC_CONFIG: i32 = -7;

#[allow(unsafe_code, reason = "the public C entrypoint requires a C symbol")]
#[unsafe(no_mangle)]
pub extern "C" fn vutnet_app_listen_v1(port: i32, route_entries: u64) -> i32 {
    catch_run(|| run(port, DEFAULT_BODY_LIMIT_BYTES, route_entries, None))
}

#[allow(unsafe_code, reason = "the public C entrypoint requires a C symbol")]
#[unsafe(no_mangle)]
pub extern "C" fn vutnet_app_listen_v2(port: i32, max_body_bytes: i32, route_entries: u64) -> i32 {
    catch_run(|| run(port, max_body_bytes, route_entries, None))
}

const DEFAULT_BODY_LIMIT_BYTES: i32 = 1024 * 1024;

#[unsafe(no_mangle)]
pub extern "C" fn vutnet_app_listen_v3(
    port: i32,
    max_body_bytes: i32,
    route_entries: u64,
    mounts: u64,
) -> i32 {
    catch_run(|| run(port, max_body_bytes, route_entries, Some(mounts)))
}

fn catch_run(run: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|_| {
        terminal::error("Native server failed unexpectedly.");
        STATUS_RUNTIME
    })
}

fn run(port: i32, max_body_bytes: i32, route_entries: u64, mounts: Option<u64>) -> i32 {
    let started = Instant::now();
    let Ok(port) = u16::try_from(port) else {
        terminal::error("Invalid port number.");
        return STATUS_INVALID_PORT;
    };
    let Ok(max_body_bytes) = usize::try_from(max_body_bytes) else {
        terminal::error("Invalid request body limit.");
        return STATUS_INVALID_BODY_LIMIT;
    };
    let Ok(runtime) = Runtime::current() else {
        terminal::error("Vut runtime is unavailable.");
        return STATUS_ABI;
    };
    let Ok(info) = Runtime::abi_info() else {
        terminal::error("Could not read the Vut native ABI information.");
        return STATUS_ABI;
    };
    let required = VUT_FEATURE_CALLBACK_SYNC_V1
        | VUT_FEATURE_CALLBACK_ASYNC_V1
        | VUT_FEATURE_DATA_FIELDS_V1
        | VUT_FEATURE_VALUE_LISTS_V1
        | VUT_FEATURE_CALLABLE_FIELDS_V1;
    if info.feature_bits & required != required {
        terminal::error("The Vut runtime does not support the required callback ABI features.");
        return STATUS_ABI;
    }

    let Ok(entries) = identity::borrowed_list::<identity::RouteEntryType>(&runtime, route_entries)
    else {
        terminal::error("Could not read the Vut router table through the public native ABI.");
        return STATUS_ABI;
    };
    let routes = match RouteTable::from_entries(&entries) {
        Ok(routes) => routes,
        Err(error) => {
            terminal::error(format!("Could not load Vut router table: {error}"));
            return STATUS_ROUTER;
        }
    };
    if std::env::var_os("VUTNET_CLI_ROUTE_DUMP").is_some_and(|value| value == "1") {
        return match crate::route_dump::write(&entries) {
            Ok(()) => STATUS_OK,
            Err(error) => {
                terminal::error(format!(
                    "Could not print the flattened Vut router table: {error}"
                ));
                STATUS_ROUTER
            }
        };
    }
    drop(entries);

    let static_files = match load_static_files(&runtime, mounts) {
        Ok(files) => files,
        Err(status) => return status,
    };

    let Ok(tokio) = Builder::new_multi_thread().enable_all().build() else {
        terminal::error("Could not start the Tokio runtime.");
        return STATUS_RUNTIME;
    };
    tokio.block_on(async move {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let config = ServerConfig::bind(address).with_max_body_bytes(max_body_bytes);
        let server = match Server::bind(config).await {
            Ok(server) => server,
            Err(error) => {
                terminal::error(format!("Failed to bind server: {error}"));
                return STATUS_BIND;
            }
        };
        let address = match server.local_addr() {
            Ok(address) => address,
            Err(error) => {
                terminal::error(format!("Could not read the server address: {error}"));
                return STATUS_BIND;
            }
        };
        terminal::server_ready(address, started.elapsed());
        let handler = VutHandler::new(runtime, routes, static_files);
        match serve_until_shutdown(server, handler).await {
            Ok(()) => {
                terminal::server_stopped();
                STATUS_OK
            }
            Err(error) => {
                terminal::error(format!("Server failed: {error}"));
                STATUS_RUNTIME
            }
        }
    })
}

fn load_static_files(
    runtime: &Runtime,
    mounts: Option<u64>,
) -> Result<crate::static_files::StaticFiles, i32> {
    let mut files = if let Some(raw) = mounts {
        let entries =
            identity::borrowed_list::<identity::StaticMountType>(runtime, raw).map_err(|_| {
                terminal::error("Could not read static file mounts through the public native ABI.");
                STATUS_ABI
            })?;
        crate::static_files::StaticFiles::from_entries(&entries).map_err(|()| {
            terminal::error("The static file mount configuration is invalid.");
            STATUS_STATIC_CONFIG
        })?
    } else {
        crate::static_files::StaticFiles::default()
    };
    if let Some(directory) = crate::browser_client::asset_directory() {
        files.add_directory(crate::browser_client::ASSET_MOUNT_PREFIX, &directory).map_err(|()| {
            terminal::error("The browser client asset mount is invalid or conflicts with an application mount.");
            STATUS_STATIC_CONFIG
        })?;
    }
    Ok(files)
}

async fn serve_until_shutdown<H>(
    server: Server,
    handler: H,
) -> Result<(), server_native::ServerError>
where
    H: server_native::Handler,
{
    let shutdown = ShutdownHandle::new();
    let signal = shutdown.clone();
    tokio::spawn(async move {
        crate::lifecycle::wait_for_shutdown_signal(signal).await;
    });
    server.serve_with_shutdown(handler, shutdown).await
}
