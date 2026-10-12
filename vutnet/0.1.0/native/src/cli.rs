//! Rust implementation behind Vutnet's Vut `[[bin]]` entrypoint.

use crate::{commands, ui::Ui};
use anyhow::Result;

const VUTNET_VERSION: &str = env!("VUTNET_VERSION");

#[must_use]
pub fn new(name: &str) -> i32 {
    execute(|ui| commands::new(name, ui))
}

#[must_use]
pub fn dev(verbose: bool) -> i32 {
    execute(|ui| commands::dev(ui, verbose))
}

#[must_use]
pub fn build() -> i32 {
    execute(commands::build)
}

#[must_use]
pub fn run() -> i32 {
    execute(commands::run)
}

#[must_use]
pub fn start() -> i32 {
    execute(commands::start)
}

#[must_use]
pub fn routes(json: bool) -> i32 {
    execute(|ui| commands::routes(json, ui))
}

#[must_use]
pub fn info() -> i32 {
    execute(commands::info)
}

#[must_use]
pub fn help() -> i32 {
    Ui::stdout(HELP);
    0
}

#[must_use]
pub fn version() -> i32 {
    Ui::stdout(&format!("vutnet {VUTNET_VERSION}\n"));
    0
}

#[must_use]
pub fn unknown_command(command: &str) -> i32 {
    let ui = Ui::new();
    ui.error(&format!("unknown command `{command}`"));
    Ui::stderr(HELP);
    2
}

#[must_use]
pub fn invalid_arguments() -> i32 {
    let ui = Ui::new();
    ui.error("invalid command arguments");
    Ui::stderr(HELP);
    2
}

fn execute(command: impl FnOnce(&Ui) -> Result<()>) -> i32 {
    let ui = Ui::new();
    match command(&ui) {
        Ok(()) => 0,
        Err(error) => {
            ui.error(&format!("{error:#}"));
            1
        }
    }
}

const HELP: &str = "Usage: vutnet <COMMAND>\n\nCommands:\n  new <name>       Create a Vutnet application\n  dev [--verbose]  Build, serve, and rebuild on source changes\n  run              Build and run the development application\n  build            Build the production application\n  start            Run the production application\n  routes           List flattened routes (--json for JSON output)\n  info             Show project and toolchain information\n  help             Show this help\n  --version        Show the Vutnet package version\n";
