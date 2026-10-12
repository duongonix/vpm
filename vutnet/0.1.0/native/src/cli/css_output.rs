use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CssPublication {
    candidate: PathBuf,
    output: PathBuf,
    backup: PathBuf,
    had_previous: bool,
    published: bool,
}

impl CssPublication {
    pub fn new(candidate: PathBuf, output: PathBuf, backup: PathBuf) -> Self {
        Self {
            candidate,
            output,
            backup,
            had_previous: false,
            published: false,
        }
    }

    pub fn publish(&mut self) -> Result<()> {
        if !self.candidate.is_file() {
            bail!("CSS candidate does not exist: {}", self.candidate.display());
        }
        let parent = self
            .output
            .parent()
            .context("CSS output path has no parent directory")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create CSS output directory {}", parent.display()))?;
        if self.output.exists() && !self.output.is_file() {
            bail!("CSS output path is not a file: {}", self.output.display());
        }

        self.had_previous = self.output.is_file();
        if self.had_previous {
            if let Some(backup_parent) = self.backup.parent() {
                fs::create_dir_all(backup_parent).with_context(|| {
                    format!(
                        "cannot create CSS backup directory {}",
                        backup_parent.display()
                    )
                })?;
            }
            fs::copy(&self.output, &self.backup).with_context(|| {
                format!(
                    "cannot preserve previous CSS output {}",
                    self.output.display()
                )
            })?;
        }

        let replacement = write_temporary(&self.candidate, parent)?;
        if let Err(error) = replacement.persist(&self.output) {
            let _ = fs::remove_file(&self.backup);
            return Err(error.error)
                .with_context(|| format!("cannot publish CSS output {}", self.output.display()));
        }
        self.published = true;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if !self.published {
            return Ok(());
        }
        if self.had_previous {
            let parent = self
                .output
                .parent()
                .context("CSS output path has no parent directory")?;
            let replacement = write_temporary(&self.backup, parent)?;
            replacement.persist(&self.output).map_err(|error| {
                anyhow::anyhow!(error.error).context(format!(
                    "cannot restore previous CSS output {}",
                    self.output.display()
                ))
            })?;
        } else {
            match fs::remove_file(&self.output) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("cannot remove failed CSS output {}", self.output.display())
                    });
                }
            }
        }
        self.published = false;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.had_previous {
            match fs::remove_file(&self.backup) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("cannot remove CSS backup {}", self.backup.display())
                    });
                }
            }
        }
        Ok(())
    }
}

fn write_temporary(candidate: &Path, directory: &Path) -> Result<tempfile::NamedTempFile> {
    let mut file = tempfile::NamedTempFile::new_in(directory)
        .with_context(|| format!("cannot create temporary output in {}", directory.display()))?;
    let mut source = File::open(candidate)
        .with_context(|| format!("cannot read CSS candidate {}", candidate.display()))?;
    io::copy(&mut source, &mut file)?;
    file.as_file().sync_all()?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn publishes_candidate_and_rolls_back_last_known_good_css() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("public/assets/app.css");
        let candidate_path = temp.path().join(".vut/dev/generation-2/assets/app.css");
        let backup = temp.path().join(".vut/dev/generation-2/previous.css");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::create_dir_all(candidate_path.parent().unwrap()).unwrap();
        fs::write(&output, b"css v1").unwrap();
        fs::write(&candidate_path, b"css v2").unwrap();

        let mut publication = CssPublication::new(candidate_path, output.clone(), backup.clone());
        publication.publish().unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"css v2");
        assert_eq!(fs::read(&backup).unwrap(), b"css v1");
        publication.rollback().unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"css v1");
    }

    #[test]
    fn rollback_removes_new_output_when_there_was_no_previous_css() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("public/app.css");
        let candidate = temp.path().join("candidate.css");
        fs::write(&candidate, b"css").unwrap();
        let mut publication =
            CssPublication::new(candidate, output.clone(), temp.path().join("previous.css"));
        publication.publish().unwrap();
        publication.rollback().unwrap();
        assert!(!output.exists());
    }

    #[test]
    fn incomplete_candidate_never_changes_current_output() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("public/app.css");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"css v1").unwrap();
        let mut publication = CssPublication::new(
            temp.path().join("missing.css"),
            output.clone(),
            temp.path().join("previous.css"),
        );
        assert!(publication.publish().is_err());
        assert_eq!(fs::read(output).unwrap(), b"css v1");
    }
}
