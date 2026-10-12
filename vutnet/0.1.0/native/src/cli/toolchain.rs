use std::{
    io::{self, BufRead, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, bail};

use crate::{
    process::ChildProcess,
    project::{Project, artifact_path},
};

pub fn create_project(parent: &Path, name: &str) -> Result<()> {
    run_vpm(["new", name], parent).context("VPM could not create the project")
}

pub fn update_project(root: &Path) -> Result<()> {
    run_vpm(["update"], root).context("VPM could not resolve project dependencies")
}

pub fn resolve_tool(
    name: &str,
    version: &str,
    cwd: &Path,
    mut on_progress: impl FnMut(u8),
) -> Result<PathBuf> {
    let executable = std::env::var_os("VUTNET_VPM").unwrap_or_else(|| "vpm".into());
    let mut child = Command::new(executable)
        .args(["--color", "never", "tool", "resolve", name, version])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("VPM (`vpm`) is required to resolve the requested build tool")?;

    let stderr = child
        .stderr
        .take()
        .context("VPM tool resolver stderr was not captured")?;
    let mut details = String::new();
    for line in io::BufReader::new(stderr).lines() {
        let line = line.context("failed to read VPM tool resolver progress")?;
        if let Some(percent) = parse_tool_progress(&line) {
            on_progress(percent);
        } else if !line.trim().is_empty() {
            details.push_str(line.trim());
            details.push('\n');
        }
    }

    let mut stdout = Vec::new();
    if let Some(mut output) = child.stdout.take() {
        output
            .read_to_end(&mut stdout)
            .context("failed to read VPM tool resolver output")?;
    }
    let status = child
        .wait()
        .context("failed to wait for VPM tool resolver")?;
    if !status.success() {
        let stdout = String::from_utf8_lossy(&stdout);
        let details = if details.trim().is_empty() {
            stdout.trim()
        } else {
            details.trim()
        };
        if details.is_empty() {
            bail!("VPM tool resolver exited with {status}");
        }
        bail!("VPM tool resolver failed: {details}");
    }
    parse_tool_path(&stdout, name, version)
}

fn parse_tool_progress(line: &str) -> Option<u8> {
    let line = line.trim_start().strip_prefix("Downloading ")?;
    let percent = line.split_whitespace().last()?.strip_suffix('%')?;
    let percent = percent.parse::<u8>().ok()?;
    (percent <= 100).then_some(percent)
}

fn parse_tool_path(stdout: &[u8], name: &str, version: &str) -> Result<PathBuf> {
    let output = std::str::from_utf8(stdout)
        .with_context(|| format!("VPM returned a non-UTF-8 path for `{name}@{version}`"))?;
    let path = output.trim();
    if path.is_empty() || path.lines().count() != 1 {
        bail!("VPM returned an invalid executable path for `{name}@{version}`");
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() || !path.is_file() {
        bail!(
            "VPM returned a missing or non-absolute executable path for `{name}@{version}`: {}",
            path.display()
        );
    }
    Ok(path)
}

pub fn build_project(project: &Project, release: bool) -> Result<std::path::PathBuf> {
    let mut args = vec!["build"];
    if release {
        args.push("--release");
    }
    run_vpm(args, &project.root).context("VPM build failed")?;

    let profile = if release { "release" } else { "debug" };
    let artifact = artifact_path(&project.root, profile);
    if !artifact.is_file() {
        bail!(
            "VPM build completed without producing the expected artifact at {}",
            artifact.display()
        );
    }
    Ok(artifact)
}

/// Builds the generated, typed Vutnet browser registry through VPM.
pub fn build_hydration_registry(project: &Project, release: bool) -> Result<Option<PathBuf>> {
    let mut args = vec![
        "build",
        "--hydration-registry",
        "--target",
        "wasm32-unknown-unknown",
    ];
    if release {
        args.push("--release");
    }
    run_vpm(args, &project.root).context("VPM browser client build failed")?;
    let artifact = hydration_artifact(project, release);
    Ok(artifact.is_file().then_some(artifact))
}

pub fn build_development(project: &Project, stopping: &AtomicBool) -> Result<bool> {
    run_vpm_interruptible(["build"], &project.root, stopping)
}

/// Builds the generated browser registry while remaining interruptible by Ctrl+C.
pub fn build_hydration_registry_development(
    project: &Project,
    stopping: &AtomicBool,
) -> Result<bool> {
    let completed = run_vpm_interruptible(
        [
            "build",
            "--hydration-registry",
            "--target",
            "wasm32-unknown-unknown",
        ],
        &project.root,
        stopping,
    )?;
    Ok(completed)
}

fn hydration_artifact(project: &Project, release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    project
        .root
        .join("target/wasm32-unknown-unknown")
        .join(profile)
        .join("vutnet-client.wasm")
}

fn run_vpm<'a>(args: impl IntoIterator<Item = &'a str>, cwd: &Path) -> Result<()> {
    let executable = std::env::var_os("VUTNET_VPM").unwrap_or_else(|| "vpm".into());
    let output = Command::new(executable)
        .args(["--quiet", "--color", "never"])
        .args(args)
        .current_dir(cwd)
        .output()
        .context("VPM (`vpm`) is required and must be available on PATH")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let details = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if details.is_empty() {
        bail!("VPM exited with {}", output.status);
    }
    bail!("{details}");
}

fn run_vpm_interruptible<'a>(
    args: impl IntoIterator<Item = &'a str>,
    cwd: &Path,
    stopping: &AtomicBool,
) -> Result<bool> {
    let executable = std::env::var_os("VUTNET_VPM").unwrap_or_else(|| "vpm".into());
    let mut command = Command::new(executable);
    command
        .args(["--quiet", "--color", "never"])
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_command_interruptible(command, "VPM build", stopping)
        .context("VPM (`vpm`) is required and must be available on PATH")
}

pub fn run_command_interruptible(
    mut command: Command,
    label: &str,
    stopping: &AtomicBool,
) -> Result<bool> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child =
        ChildProcess::spawn(command).with_context(|| format!("failed to start {label}"))?;
    let stdout = child
        .take_stdout()
        .with_context(|| format!("{label} stdout was not piped"))?;
    let stderr = child
        .take_stderr()
        .with_context(|| format!("{label} stderr was not piped"))?;
    let stdout_reader = spawn_reader(stdout);
    let stderr_reader = spawn_reader(stderr);

    loop {
        if stopping.load(Ordering::Acquire) {
            child
                .try_terminate()
                .with_context(|| format!("failed to stop the {label} process tree"))?;
            let _ = join_reader(stdout_reader);
            let _ = join_reader(stderr_reader);
            return Ok(false);
        }
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("failed to poll {label}"))?
        {
            let stdout = join_reader(stdout_reader)?;
            let stderr = join_reader(stderr_reader)?;
            if status.success() {
                return Ok(true);
            }
            let stderr = String::from_utf8_lossy(&stderr);
            let stdout = String::from_utf8_lossy(&stdout);
            let details = if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            };
            if details.is_empty() {
                anyhow::bail!("{label} exited with {status}");
            }
            anyhow::bail!("{label} failed:\n{details}");
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn spawn_reader<R: Read + Send + 'static>(mut reader: R) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_reader(reader: JoinHandle<io::Result<Vec<u8>>>) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("child output reader thread panicked"))?
        .context("failed to read child process output")
}

#[cfg(test)]
mod tests {
    use super::{parse_tool_path, parse_tool_progress};

    #[test]
    fn parses_only_vpm_download_percentage_lines() {
        assert_eq!(
            parse_tool_progress(" Downloading tailwindcss.exe 35%"),
            Some(35)
        );
        assert_eq!(parse_tool_progress("   Downloading tool 100%"), Some(100));
        assert_eq!(parse_tool_progress(" Resolving tailwindcss 35%"), None);
        assert_eq!(parse_tool_progress(" Downloading tool 101%"), None);
        assert_eq!(parse_tool_progress(" Downloading tool"), None);
    }

    #[test]
    fn artifact_selection_matches_vpm_profiles() {
        let root = std::path::Path::new("sample");
        assert_eq!(
            super::artifact_path(root, "release"),
            root.join("build/release")
                .join(if cfg!(windows) { "app.exe" } else { "app" })
        );
    }

    #[test]
    fn resolved_tool_output_must_be_one_existing_absolute_path() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("tool.exe");
        std::fs::write(&executable, b"tool").unwrap();
        assert_eq!(
            parse_tool_path(
                format!("{}\n", executable.display()).as_bytes(),
                "tailwindcss",
                "4.1.14"
            )
            .unwrap(),
            executable
        );
        assert!(parse_tool_path(b"tool.exe\n", "tailwindcss", "4.1.14").is_err());
        assert!(parse_tool_path(b"C:\\tool.exe\nextra\n", "tailwindcss", "4.1.14").is_err());
    }
}
