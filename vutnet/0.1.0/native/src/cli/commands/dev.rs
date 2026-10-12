mod artifact;
mod builder;
mod runtime;
mod supervisor;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result};

use crate::{
    config::VutnetConfig, project::Project, tailwind::ResolvedTailwind, ui::Ui,
    watcher::FileWatcher,
};

use self::{runtime::NativeDevRuntime, supervisor::DevSupervisor};

pub(super) fn run(ui: &Ui, verbose_requests: bool) -> Result<()> {
    let project = Project::discover(&std::env::current_dir()?)?;
    let config = VutnetConfig::load(&project.root)?;
    let tailwind = ResolvedTailwind::resolve(&project.root, config.tailwind, ui)?;
    let dependencies = project.local_dependency_roots()?;
    let watcher = match tailwind.as_ref() {
        Some(tool) => {
            let css_output = tool.output_path(&project.root);
            FileWatcher::new_with_tailwind(&project.root, &dependencies, Some(&css_output))
        }
        None => FileWatcher::new(&project.root, &dependencies),
    }
    .context("failed to start the development file watcher")?;
    let stopping = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stopping);
    ctrlc::set_handler(move || signal.store(true, Ordering::Release))
        .context("could not install Ctrl+C handler")?;

    DevSupervisor::new(
        NativeDevRuntime::new(project, tailwind, verbose_requests),
        ui,
    )
    .run(&watcher, &stopping)
}
