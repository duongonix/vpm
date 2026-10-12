use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::css_output::CssPublication;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CandidateArtifact {
    directory: PathBuf,
    path: PathBuf,
    css: Option<CssPublication>,
    browser_directory: Option<PathBuf>,
    #[cfg(test)]
    generation: u64,
}

impl CandidateArtifact {
    pub fn copy_from(root: &Path, source: &Path, generation: u64) -> Result<Self> {
        let directory = root.join(".vut/dev");
        fs::create_dir_all(&directory)
            .with_context(|| format!("cannot create {}", directory.display()))?;

        let process_id = std::process::id();
        let mut suffix = 0u32;
        let generation_directory = loop {
            let candidate = directory.join(generation_name(process_id, generation, suffix));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    suffix = suffix.saturating_add(1);
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("cannot create candidate generation {}", candidate.display())
                    });
                }
            }
        };

        let path = generation_directory.join(application_name());
        if let Err(error) = fs::copy(source, &path) {
            let _ = fs::remove_dir_all(&generation_directory);
            return Err(error)
                .with_context(|| format!("cannot copy build candidate to {}", path.display()));
        }
        Ok(Self {
            directory: generation_directory,
            path,
            css: None,
            browser_directory: None,
            #[cfg(test)]
            generation,
        })
    }

    pub fn set_css_candidate(&mut self, candidate: PathBuf, output: PathBuf) {
        self.css = Some(CssPublication::new(
            candidate,
            output,
            self.directory.join("previous-app.css"),
        ));
    }

    pub fn copy_browser_client(&mut self, root: &Path) -> Result<()> {
        self.browser_directory = crate::browser_assets::stage(root, &self.directory, false)?;
        Ok(())
    }

    pub fn css_candidate_path(&self) -> PathBuf {
        self.directory.join("assets/app.css")
    }

    pub fn publish_css(&mut self) -> Result<()> {
        if let Some(css) = &mut self.css {
            css.publish()?;
        }
        Ok(())
    }

    pub fn rollback_css(&mut self) -> Result<()> {
        if let Some(css) = &mut self.css {
            css.rollback()?;
        }
        Ok(())
    }

    pub fn finish_css(&mut self) -> Result<()> {
        if let Some(css) = &mut self.css {
            css.finish()?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn browser_directory(&self) -> Option<&Path> {
        self.browser_directory.as_deref()
    }

    #[cfg(test)]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn cleanup(&self) -> Result<()> {
        match fs::remove_dir_all(&self.directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("cannot remove {}", self.directory.display()))
            }
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(path: PathBuf, generation: u64) -> Self {
        let directory = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Self {
            directory,
            path,
            css: None,
            browser_directory: None,
            generation,
        }
    }
}

fn generation_name(process_id: u32, generation: u64, suffix: u32) -> String {
    format!("generation-{process_id}-{generation:06}-{suffix}")
}

fn application_name() -> String {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    format!("app{extension}")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    #[test]
    fn candidate_uses_generation_path_and_cleans_up() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp
            .path()
            .join(if cfg!(windows) { "app.exe" } else { "app" });
        fs::write(&source, b"candidate executable").unwrap();

        let candidate = CandidateArtifact::copy_from(temp.path(), &source, 12).unwrap();
        assert_eq!(candidate.generation(), 12);
        assert!(candidate.path().starts_with(temp.path().join(".vut/dev")));
        assert_eq!(fs::read(candidate.path()).unwrap(), b"candidate executable");
        if cfg!(windows) {
            assert_eq!(
                candidate.path().extension().and_then(|ext| ext.to_str()),
                Some("exe")
            );
        } else {
            assert_eq!(candidate.path().extension(), None);
        }

        candidate.cleanup().unwrap();
        assert!(!candidate.directory.exists());
        assert!(Path::new(&source).is_file());
    }

    #[test]
    fn repeated_generation_never_overwrites_an_existing_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("build-output");
        fs::write(&source, b"candidate").unwrap();

        let first = CandidateArtifact::copy_from(temp.path(), &source, 1).unwrap();
        let second = CandidateArtifact::copy_from(temp.path(), &source, 1).unwrap();
        assert_ne!(first.path(), second.path());
        assert_eq!(fs::read(first.path()).unwrap(), b"candidate");
        assert_eq!(fs::read(second.path()).unwrap(), b"candidate");
    }
}
