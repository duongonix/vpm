use std::{path::Path, process::Command};

use anyhow::Result;

use crate::{project::Project, ui::Ui};

pub(super) fn run(ui: &Ui) -> Result<()> {
    let project = Project::discover(&std::env::current_dir()?)?;
    ui.header();
    ui.info_table(&[
        ("Vutnet CLI", env!("CARGO_PKG_VERSION").to_owned()),
        (
            "Vutnet package",
            project
                .vutnet_dependency_version()
                .unwrap_or("unknown")
                .to_owned(),
        ),
        ("Project", project.package_name().to_owned()),
        ("Vut", tool_version("vut")),
        ("VPM", tool_version("vpm")),
        (
            "Manifest",
            relative(&project.root.join("vpm.toml"), &std::env::current_dir()?),
        ),
        ("Entry", relative(&project.entry_path(), &project.root)),
        ("Router", router_path(&project.root)),
        (
            "Host",
            format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        ),
    ]);
    Ok(())
}

fn tool_version(program: &str) -> String {
    Command::new(program)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn router_path(root: &Path) -> String {
    ["src/router.vut", "src/router/mod.vut"]
        .into_iter()
        .find(|path| root.join(path).is_file())
        .unwrap_or("unknown")
        .to_owned()
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_optional_router_is_reported_without_guessing() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(router_path(temp.path()), "unknown");
    }
}
