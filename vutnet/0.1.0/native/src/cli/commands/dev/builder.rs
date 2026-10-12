use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};

use crate::{
    project::{Project, artifact_path},
    tailwind::ResolvedTailwind,
    toolchain,
};

use super::artifact::CandidateArtifact;

pub(super) enum BuildCandidate {
    Ready(Box<CandidateArtifact>),
    Cancelled,
}

pub(super) fn build_candidate(
    project: &Project,
    generation: u64,
    stopping: &AtomicBool,
    tailwind: Option<&ResolvedTailwind>,
) -> Result<BuildCandidate> {
    if !toolchain::build_development(project, stopping)? {
        return Ok(BuildCandidate::Cancelled);
    }
    if stopping.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(BuildCandidate::Cancelled);
    }

    let output = artifact_path(&project.root, "debug");
    if !output.is_file() {
        bail!(
            "VPM build completed without producing the expected artifact at {}",
            output.display()
        );
    }
    let mut candidate = CandidateArtifact::copy_from(&project.root, &output, generation)
        .with_context(|| format!("failed to preserve candidate from {}", output.display()))?;
    match toolchain::build_hydration_registry_development(project, stopping) {
        Ok(true) => {
            if let Err(error) = candidate.copy_browser_client(&project.root) {
                let _ = candidate.cleanup();
                return Err(error).context("failed to stage the browser client");
            }
        }
        Ok(false) => {
            let _ = candidate.cleanup();
            return Ok(BuildCandidate::Cancelled);
        }
        Err(error) => {
            let _ = candidate.cleanup();
            return Err(error).context("VPM browser client build failed");
        }
    }
    if let Some(tailwind) = tailwind {
        let css_candidate = candidate.css_candidate_path();
        match tailwind.compile_candidate_interruptible(
            &project.root,
            &css_candidate,
            false,
            stopping,
        ) {
            Ok(true) => {}
            Ok(false) => {
                let _ = candidate.cleanup();
                return Ok(BuildCandidate::Cancelled);
            }
            Err(error) => {
                let _ = candidate.cleanup();
                return Err(error).context("Tailwind CSS candidate failed");
            }
        }
        candidate.set_css_candidate(css_candidate, tailwind.output_path(&project.root));
    }
    if stopping.load(std::sync::atomic::Ordering::Acquire) {
        let _ = candidate.cleanup();
        return Ok(BuildCandidate::Cancelled);
    }
    Ok(BuildCandidate::Ready(Box::new(candidate)))
}
