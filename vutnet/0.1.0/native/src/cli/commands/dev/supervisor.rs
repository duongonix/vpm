use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use crate::{ui::Ui, watcher::FileWatcher};

use super::artifact::CandidateArtifact;

pub(super) struct ServerExit {
    pub success: bool,
    pub display: String,
}

pub(super) trait DevRuntime {
    type Process;

    fn build_candidate(&mut self, stopping: &AtomicBool) -> Result<Option<CandidateArtifact>>;
    fn spawn_server(&mut self, artifact: &CandidateArtifact) -> Result<Self::Process>;
    fn try_wait(&mut self, process: &mut Self::Process) -> Result<Option<ServerExit>>;
    fn stop(&mut self, process: &mut Self::Process) -> Result<()>;
    fn cleanup_artifact(&mut self, artifact: &CandidateArtifact) -> Result<()>;

    fn publish_assets(&mut self, artifact: &mut CandidateArtifact) -> Result<()> {
        artifact.publish_css()
    }

    fn rollback_assets(&mut self, artifact: &mut CandidateArtifact) -> Result<()> {
        artifact.rollback_css()
    }

    fn finish_assets(&mut self, artifact: &mut CandidateArtifact) -> Result<()> {
        artifact.finish_css()
    }
}

struct RunningServer<P> {
    process: P,
}

pub(super) struct DevSupervisor<'a, R: DevRuntime> {
    runtime: R,
    ui: &'a Ui,
    server: Option<RunningServer<R::Process>>,
    last_successful: Option<CandidateArtifact>,
}

impl<'a, R: DevRuntime> DevSupervisor<'a, R> {
    pub fn new(runtime: R, ui: &'a Ui) -> Self {
        Self {
            runtime,
            ui,
            server: None,
            last_successful: None,
        }
    }

    pub fn run(&mut self, watcher: &FileWatcher, stopping: &AtomicBool) -> Result<()> {
        self.ui.header();
        self.rebuild(|| watcher.change_revision(), stopping);

        let result = self.event_loop(watcher, stopping);
        let shutdown = self.shutdown();
        result?;
        shutdown
    }

    fn event_loop(&mut self, watcher: &FileWatcher, stopping: &AtomicBool) -> Result<()> {
        while !stopping.load(Ordering::Acquire) {
            self.poll_server()?;
            let Some(path) = watcher.next_change(Duration::from_millis(100))? else {
                continue;
            };
            let shown = path
                .strip_prefix(watcher.project_root())
                .unwrap_or(&path)
                .to_string_lossy();
            self.ui.changed(&shown);
            self.rebuild(|| watcher.change_revision(), stopping);
        }
        Ok(())
    }

    fn rebuild(&mut self, revision: impl Fn() -> u64, stopping: &AtomicBool) {
        if stopping.load(Ordering::Acquire) {
            return;
        }
        let rebuilding = self.server.is_some();
        self.ui.progress(if rebuilding {
            "Rebuilding..."
        } else {
            "Building development application..."
        });
        let build_revision = revision();
        let started = Instant::now();
        let candidate = match self.runtime.build_candidate(stopping) {
            Ok(Some(candidate)) => candidate,
            Ok(None) => return,
            Err(error) => {
                if stopping.load(Ordering::Acquire) {
                    return;
                }
                if revision() != build_revision {
                    self.ui
                        .progress("Source changed while compiling; waiting for the latest save...");
                    return;
                }
                self.compile_failed(&error);
                return;
            }
        };

        if stopping.load(Ordering::Acquire) {
            self.cleanup(&candidate);
            return;
        }
        if revision() != build_revision {
            self.cleanup(&candidate);
            self.ui
                .progress("Source changed while compiling; waiting for the latest save...");
            return;
        }
        self.ui
            .status(&format!("Rebuilt in {}ms", started.elapsed().as_millis()));
        self.replace_server(candidate);
    }

    fn replace_server(&mut self, mut candidate: CandidateArtifact) {
        let previous_success = self.last_successful.clone();
        if let Some(mut previous) = self.server.take()
            && let Err(error) = self.runtime.stop(&mut previous.process)
        {
            self.server = Some(previous);
            self.cleanup(&candidate);
            self.ui
                .error(&format!("Could not stop the current server\n\n{error:#}"));
            self.ui
                .progress("Keeping previous server; waiting for changes...");
            return;
        }

        if let Err(error) = self.runtime.publish_assets(&mut candidate) {
            self.cleanup(&candidate);
            self.ui
                .error(&format!("Could not publish candidate assets\n\n{error:#}"));
            self.restart_previous(previous_success.as_ref());
            self.ui.progress("Waiting for changes...");
            return;
        }

        match self.runtime.spawn_server(&candidate) {
            Ok(process) => {
                if let Err(error) = self.runtime.finish_assets(&mut candidate) {
                    self.ui
                        .warning(&format!("could not finalize CSS publication: {error:#}"));
                }
                self.server = Some(RunningServer { process });
                self.last_successful = Some(candidate.clone());
                if let Some(previous) = previous_success
                    && previous.path() != candidate.path()
                {
                    self.cleanup(&previous);
                }
            }
            Err(error) => {
                let rollback = self.runtime.rollback_assets(&mut candidate);
                self.cleanup(&candidate);
                self.ui
                    .error(&format!("Server failed to start\n\n{error:#}"));
                match rollback {
                    Ok(()) => self.restart_previous(previous_success.as_ref()),
                    Err(rollback_error) => self.ui.error(&format!(
                        "Could not restore previous CSS output\n\n{rollback_error:#}"
                    )),
                }
                self.ui.progress("Waiting for changes...");
            }
        }
    }

    fn restart_previous(&mut self, previous: Option<&CandidateArtifact>) {
        let Some(previous) = previous else {
            return;
        };
        match self.runtime.spawn_server(previous) {
            Ok(process) => self.server = Some(RunningServer { process }),
            Err(error) => self.ui.error(&format!(
                "Could not restart last-known-good server\n\n{error:#}"
            )),
        }
    }

    fn compile_failed(&mut self, error: &anyhow::Error) {
        self.ui.error(&format!("Compile failed\n\n{error:#}"));
        if self.server.is_some() {
            self.ui.progress("Keeping previous server");
        }
        self.ui.progress("Waiting for changes...");
    }

    fn poll_server(&mut self) -> Result<()> {
        let Some(server) = &mut self.server else {
            return Ok(());
        };
        let Some(exit) = self
            .runtime
            .try_wait(&mut server.process)
            .context("failed to monitor the development server")?
        else {
            return Ok(());
        };

        let mut exited = self.server.take().expect("server was just observed");
        if let Err(error) = self.runtime.stop(&mut exited.process) {
            self.ui
                .warning(&format!("process tree cleanup failed: {error:#}"));
        }
        if exit.success {
            self.ui
                .warning(&format!("Server exited unexpectedly ({})", exit.display));
        } else {
            self.ui
                .error(&format!("Server exited unexpectedly ({})", exit.display));
        }
        self.ui.progress("Waiting for changes...");
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        if let Some(mut server) = self.server.take() {
            self.runtime
                .stop(&mut server.process)
                .context("failed to stop the development server process tree")?;
        }
        if let Some(artifact) = self.last_successful.take() {
            self.cleanup(&artifact);
        }
        self.ui.status("Server stopped");
        Ok(())
    }

    fn cleanup(&mut self, artifact: &CandidateArtifact) {
        if let Err(error) = self.runtime.cleanup_artifact(artifact) {
            self.ui
                .warning(&format!("could not clean build artifact: {error:#}"));
        }
    }
}

#[cfg(test)]
mod tests;
