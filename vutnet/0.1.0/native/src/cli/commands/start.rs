use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};

use crate::{bundle, process::ChildProcess, project::Project, ui::Ui};

pub(super) fn run(ui: &Ui) -> Result<()> {
    let project = Project::discover(&std::env::current_dir()?)?;
    let artifact = production_artifact(&project)?;
    ui.header();
    let mut command = Command::new(&artifact.artifact);
    artifact.configure_browser(&mut command);
    command
        .current_dir(&project.root)
        .env("VUTNET_CLI_MODE", "production");
    let mut child = ChildProcess::spawn(command).context("failed to start production server")?;
    wait_for_child(&mut child, ui)
}

fn production_artifact(project: &Project) -> Result<bundle::Bundle> {
    let directory = project
        .release_artifact()
        .parent()
        .context("artifact has no directory")?
        .join("vutnet");
    bundle::load(&directory).with_context(|| {
        format!(
            "production build not found or invalid at {}; run vutnet build",
            directory.display()
        )
    })
}

pub(super) fn wait_for_child(child: &mut ChildProcess, ui: &Ui) -> Result<()> {
    let stopping = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stopping);
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))
        .context("could not install Ctrl+C handler")?;
    loop {
        if stopping.load(Ordering::SeqCst) {
            child.terminate();
            ui.status("Server stopped");
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            bail!("server process exited with {status}");
        }
        thread::sleep(Duration::from_millis(80));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn missing_production_build_suggests_the_build_command() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("src")).unwrap();
        fs::write(
            temp.path().join("vpm.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n[dependencies]\n",
        )
        .unwrap();
        let project = Project::discover(temp.path()).unwrap();
        let error = production_artifact(&project).unwrap_err().to_string();
        assert!(error.contains("production build not found"));
        assert!(error.contains("vutnet build"));
    }
}
