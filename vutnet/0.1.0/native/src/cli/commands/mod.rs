mod build;
mod dev;
mod info;
mod new;
mod routes;
mod run;
mod start;

use crate::ui::Ui;
use anyhow::Result;

pub(super) fn build(ui: &Ui) -> Result<()> {
    build::run(ui)
}

pub(super) fn dev(ui: &Ui, verbose_requests: bool) -> Result<()> {
    dev::run(ui, verbose_requests)
}

pub(super) fn info(ui: &Ui) -> Result<()> {
    info::run(ui)
}

pub(super) fn new(name: &str, ui: &Ui) -> Result<()> {
    new::run(name, ui)
}

pub(super) fn routes(json: bool, ui: &Ui) -> Result<()> {
    routes::run(json, ui)
}

pub(super) fn run(ui: &Ui) -> Result<()> {
    run::run(ui)
}

pub(super) fn start(ui: &Ui) -> Result<()> {
    start::run(ui)
}
