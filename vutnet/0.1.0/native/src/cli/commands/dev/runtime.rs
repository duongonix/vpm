use std::{process::Command, sync::atomic::AtomicBool};

use anyhow::{Context, Result};

use crate::{
    browser_client, process::ChildProcess, project::Project, tailwind::ResolvedTailwind, terminal,
};

use super::{
    artifact::CandidateArtifact,
    builder::{self, BuildCandidate},
    supervisor::{DevRuntime, ServerExit},
};

pub(super) struct NativeDevRuntime {
    project: Project,
    next_generation: u64,
    show_startup_details: bool,
    verbose_requests: bool,
    tailwind: Option<ResolvedTailwind>,
}

impl NativeDevRuntime {
    pub fn new(
        project: Project,
        tailwind: Option<ResolvedTailwind>,
        verbose_requests: bool,
    ) -> Self {
        Self {
            project,
            next_generation: 1,
            show_startup_details: true,
            verbose_requests,
            tailwind,
        }
    }
}

impl DevRuntime for NativeDevRuntime {
    type Process = ChildProcess;

    fn build_candidate(&mut self, stopping: &AtomicBool) -> Result<Option<CandidateArtifact>> {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        match builder::build_candidate(&self.project, generation, stopping, self.tailwind.as_ref())?
        {
            BuildCandidate::Ready(candidate) => Ok(Some(*candidate)),
            BuildCandidate::Cancelled => Ok(None),
        }
    }

    fn spawn_server(&mut self, artifact: &CandidateArtifact) -> Result<Self::Process> {
        let mut command = Command::new(artifact.path());
        command
            .current_dir(&self.project.root)
            .env("VUTNET_CLI_MODE", "development")
            .env("VUTNET_CLI_WATCHING", "src/**/*.vut")
            .env(terminal::DEV_SUPERVISOR_ENV, "1")
            .env(
                terminal::DEV_REQUESTS_VERBOSE_ENV,
                if self.verbose_requests { "1" } else { "0" },
            )
            .env(
                terminal::DEV_STARTUP_DETAILS_ENV,
                if self.show_startup_details { "1" } else { "0" },
            );
        if let Some(directory) = artifact.browser_directory() {
            command.env(browser_client::ASSET_DIRECTORY_ENV, directory);
        }
        let process = ChildProcess::spawn(command).context("failed to start development server")?;
        self.show_startup_details = false;
        Ok(process)
    }

    fn try_wait(&mut self, process: &mut Self::Process) -> Result<Option<ServerExit>> {
        let status = process.try_wait()?;
        if status.is_some() {
            self.show_startup_details = true;
        }
        Ok(status.map(|status| ServerExit {
            success: status.success(),
            display: status.to_string(),
        }))
    }

    fn stop(&mut self, process: &mut Self::Process) -> Result<()> {
        process
            .try_terminate()
            .context("failed to stop the development server process tree")
    }

    fn cleanup_artifact(&mut self, artifact: &CandidateArtifact) -> Result<()> {
        artifact.cleanup()
    }
}
