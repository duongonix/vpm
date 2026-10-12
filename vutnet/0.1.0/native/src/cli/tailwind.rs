use std::sync::atomic::AtomicBool;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

use crate::{config::TailwindConfig, css_output::CssPublication, toolchain, ui::Ui};

pub(super) struct ResolvedTailwind {
    executable: PathBuf,
    config: TailwindConfig,
}

pub(super) struct StagedCss {
    _scratch: tempfile::TempDir,
    publication: CssPublication,
}

impl StagedCss {
    pub fn publish(&mut self) -> Result<()> {
        self.publication.publish()
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.publication.rollback()
    }

    pub fn finish(&mut self) -> Result<()> {
        self.publication.finish()
    }
}

impl ResolvedTailwind {
    pub fn resolve(
        project_root: &Path,
        config: Option<TailwindConfig>,
        ui: &Ui,
    ) -> Result<Option<Self>> {
        let Some(config) = config else {
            return Ok(None);
        };
        let message = format!("Resolving Tailwind CSS {} through VPM...", config.version);
        ui.begin_live_progress(&message);
        ui.update_live_progress(&format!("{message} 0%"));
        let result =
            toolchain::resolve_tool("tailwindcss", &config.version, project_root, |percent| {
                ui.update_live_progress(&format!("{message} {percent}%"));
            });
        if result.is_ok() {
            ui.update_live_progress(&format!("{message} 100%"));
        }
        ui.finish_live_progress();
        let executable =
            result.with_context(|| format!("could not resolve Tailwind CSS {}", config.version))?;
        Ok(Some(Self { executable, config }))
    }

    pub fn output_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.config.output)
    }

    pub fn stage_output(&self, project_root: &Path, minify: bool) -> Result<StagedCss> {
        let scratch_root = project_root.join(".vut/tailwind");
        std::fs::create_dir_all(&scratch_root).with_context(|| {
            format!(
                "cannot create Tailwind scratch directory {}",
                scratch_root.display()
            )
        })?;
        let scratch = tempfile::Builder::new()
            .prefix("candidate-")
            .tempdir_in(&scratch_root)
            .with_context(|| {
                format!(
                    "cannot create CSS candidate under {}",
                    scratch_root.display()
                )
            })?;
        let candidate = scratch.path().join("app.css");
        self.compile_candidate(project_root, &candidate, minify)?;
        let publication = CssPublication::new(
            candidate,
            self.output_path(project_root),
            scratch.path().join("previous.css"),
        );
        Ok(StagedCss {
            _scratch: scratch,
            publication,
        })
    }

    pub fn compile_candidate(
        &self,
        project_root: &Path,
        output: &Path,
        minify: bool,
    ) -> Result<()> {
        if !self.compile_candidate_with_stop(project_root, output, minify, None)? {
            bail!("Tailwind CSS build was cancelled");
        }
        Ok(())
    }

    pub fn compile_candidate_interruptible(
        &self,
        project_root: &Path,
        output: &Path,
        minify: bool,
        stopping: &AtomicBool,
    ) -> Result<bool> {
        self.compile_candidate_with_stop(project_root, output, minify, Some(stopping))
    }

    fn compile_candidate_with_stop(
        &self,
        project_root: &Path,
        output: &Path,
        minify: bool,
        stopping: Option<&AtomicBool>,
    ) -> Result<bool> {
        let input = project_root.join(&self.config.input);
        if !input.is_file() {
            bail!(
                "Tailwind input CSS file does not exist: {}",
                input.display()
            );
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("cannot create CSS candidate directory {}", parent.display())
            })?;
        }

        let mut command = command(&self.executable, project_root, &input, output, minify);
        if let Some(stopping) = stopping {
            if !toolchain::run_command_interruptible(
                command,
                &format!("Tailwind CSS {} build", self.config.version),
                stopping,
            )? {
                return Ok(false);
            }
        } else {
            let result = command.output().with_context(|| {
                format!(
                    "failed to start Tailwind CSS {} executable at {}",
                    self.config.version,
                    self.executable.display()
                )
            })?;
            if !result.status.success() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                let stdout = String::from_utf8_lossy(&result.stdout);
                let mut details = String::new();
                if !stderr.trim().is_empty() {
                    details.push_str(stderr.trim());
                }
                if !stdout.trim().is_empty() {
                    if !details.is_empty() {
                        details.push('\n');
                    }
                    details.push_str(stdout.trim());
                }
                if details.is_empty() {
                    bail!(
                        "Tailwind CSS {} exited with {}",
                        self.config.version,
                        result.status
                    );
                }
                bail!("Tailwind CSS build failed:\n{details}");
            }
        }
        if !output.is_file() {
            bail!(
                "Tailwind CSS completed without creating {}",
                output.display()
            );
        }
        Ok(true)
    }
}

fn command(
    executable: &Path,
    project_root: &Path,
    input: &Path,
    output: &Path,
    minify: bool,
) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--cwd")
        .arg(project_root)
        .current_dir(project_root);
    if minify {
        command.arg("--minify");
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_uses_absolute_paths_and_only_minifies_production_builds() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("app");
        let input = root.join("src/app.css");
        let output = root.join(".vut/dev/generation-1/assets/app.css");
        let debug = command(Path::new("tailwindcss.exe"), &root, &input, &output, false);
        let debug_args = debug
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            debug_args,
            [
                "--input".to_owned(),
                input.display().to_string(),
                "--output".to_owned(),
                output.display().to_string(),
                "--cwd".to_owned(),
                root.display().to_string()
            ]
        );
        let production = command(Path::new("tailwindcss.exe"), &root, &input, &output, true);
        assert_eq!(production.get_args().last().unwrap(), "--minify");
    }

    #[test]
    fn missing_or_failing_cli_does_not_write_to_active_css_output() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let input = root.join("src/app.css");
        let active_output = root.join("public/app.css");
        let candidate = root.join(".vut/dev/generation-1/app.css");
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        std::fs::create_dir_all(candidate.parent().unwrap()).unwrap();
        std::fs::create_dir_all(active_output.parent().unwrap()).unwrap();
        std::fs::write(&input, "@import 'tailwindcss';").unwrap();
        std::fs::write(&active_output, b"last-good-css").unwrap();
        let compiler = ResolvedTailwind {
            executable: root.join("missing-tailwindcss.exe"),
            config: TailwindConfig {
                version: "4.1.14".to_owned(),
                input: PathBuf::from("src/app.css"),
                output: PathBuf::from("public/app.css"),
            },
        };

        assert!(compiler.compile_candidate(root, &candidate, false).is_err());
        assert!(!candidate.exists());
        assert_eq!(std::fs::read(active_output).unwrap(), b"last-good-css");
    }
}
