use std::process::Command;

use anyhow::{Context, Result};

use crate::{
    bundle, config::VutnetConfig, process::ChildProcess, project::Project,
    tailwind::ResolvedTailwind, ui::Ui,
};

pub(super) fn run(ui: &Ui) -> Result<()> {
    let project = Project::discover(&std::env::current_dir()?)?;
    let config = VutnetConfig::load(&project.root)?;
    ui.header();
    ui.progress("Building development application...");
    let tailwind = ResolvedTailwind::resolve(&project.root, config.tailwind, ui)?;
    let mut css = tailwind
        .as_ref()
        .map(|tailwind| tailwind.stage_output(&project.root, false))
        .transpose()?;
    let started = std::time::Instant::now();
    let candidate = bundle::prepare(&project, false)?;
    if let Some(css) = &mut css {
        css.publish()?;
    }
    let built = match candidate.publish() {
        Ok(built) => built,
        Err(error) => {
            if let Some(css) = &mut css {
                css.rollback()?;
            }
            return Err(error);
        }
    };
    ui.status(&format!("Built in {}ms", started.elapsed().as_millis()));

    let mut command = Command::new(&built.artifact);
    built.configure_browser(&mut command);
    command
        .current_dir(&project.root)
        .env("VUTNET_CLI_MODE", "development");
    let mut child = match ChildProcess::spawn(command).context("failed to start application") {
        Ok(child) => child,
        Err(error) => {
            if let Some(css) = &mut css {
                css.rollback()
                    .context("could not restore CSS after application start failed")?;
            }
            return Err(error);
        }
    };
    if let Some(css) = &mut css
        && let Err(error) = css.finish()
    {
        ui.warning(&format!("could not remove CSS backup: {error:#}"));
    }
    super::start::wait_for_child(&mut child, ui)
}
