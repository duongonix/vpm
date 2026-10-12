//! Vutnet's native adapter over the public Vut callback ABI and `server` core.
//!
//! This layer owns route matching and the mapping between public Vut values
//! and the generic HTTP transport. It never reads Vut object layouts.

// Tests link an implementation of the public C symbols.
// Production packages obtain them from the compiler-linked Vut runtime.
#[cfg(test)]
extern crate vut_runtime;

#[path = "cli/browser_assets.rs"]
mod browser_assets;
#[path = "dev_client.rs"]
mod browser_client;
#[path = "cli/bundle.rs"]
mod bundle;
mod cli;
mod cli_ffi;
#[path = "cli/commands/mod.rs"]
mod commands;
#[path = "cli/config.rs"]
mod config;
mod context;
#[path = "cli/css_output.rs"]
mod css_output;
mod ffi;
mod identity;
mod lifecycle;
mod multipart;
#[path = "cli/process.rs"]
mod process;
#[path = "cli/project.rs"]
mod project;
mod request;
mod response;
mod route_dump;
mod routing;
mod static_files;
#[path = "cli/tailwind.rs"]
mod tailwind;
mod terminal;
#[path = "cli/toolchain.rs"]
mod toolchain;
#[path = "cli/ui.rs"]
mod ui;
mod view;
#[path = "cli/watcher.rs"]
mod watcher;
