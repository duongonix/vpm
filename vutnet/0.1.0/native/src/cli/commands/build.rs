use std::time::Instant;

use anyhow::Result;

use crate::{bundle, config::VutnetConfig, project::Project, tailwind::ResolvedTailwind, ui::Ui};

pub(super) fn run(ui: &Ui) -> Result<()> {
    let project = Project::discover(&std::env::current_dir()?)?;
    let config = VutnetConfig::load(&project.root)?;
    ui.header();
    ui.progress("Building production application...");
    let tailwind = ResolvedTailwind::resolve(&project.root, config.tailwind, ui)?;
    let mut css = tailwind
        .as_ref()
        .map(|tailwind| tailwind.stage_output(&project.root, true))
        .transpose()?;
    let started = Instant::now();
    let candidate = bundle::prepare(&project, true)?;
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
    let artifact = built.artifact;
    if let Some(css) = &mut css
        && let Err(error) = css.finish()
    {
        ui.warning(&format!("could not remove CSS backup: {error:#}"));
    }
    ui.status(&format!("Built in {}ms", started.elapsed().as_millis()));
    ui.detail("Artifact", &artifact.display().to_string());
    Ok(())
}
