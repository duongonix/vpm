//! Publication and validation of a complete executable/browser build.
use crate::{browser_assets, project::Project, toolchain};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    browser: bool,
}

#[derive(Debug)]
pub(crate) struct Bundle {
    pub artifact: PathBuf,
    pub browser: Option<PathBuf>,
}

impl Bundle {
    pub fn configure_browser(&self, command: &mut std::process::Command) {
        command.env_remove(crate::browser_client::ASSET_DIRECTORY_ENV);
        if let Some(directory) = &self.browser {
            command.env(crate::browser_client::ASSET_DIRECTORY_ENV, directory);
        }
    }
}
pub(crate) struct PreparedBundle {
    staging: tempfile::TempDir,
    destination: PathBuf,
}
impl PreparedBundle {
    pub fn publish(self) -> Result<Bundle> {
        load(self.staging.path()).context("staged bundle is incomplete")?;
        publish(self.staging.path(), &self.destination)?;
        load(&self.destination)
    }
}
pub(crate) fn prepare(project: &Project, release: bool) -> Result<PreparedBundle> {
    let executable = toolchain::build_project(project, release)?;
    let parent = executable
        .parent()
        .context("application artifact has no parent")?;
    let staging = tempfile::Builder::new()
        .prefix(".vutnet-build-")
        .tempdir_in(parent)?;
    let name = executable
        .file_name()
        .context("application artifact has no name")?;
    fs::copy(&executable, staging.path().join(name)).context("cannot stage server executable")?;
    toolchain::build_hydration_registry(project, release)?;
    let browser = browser_assets::stage(&project.root, staging.path(), release)?;
    fs::write(
        staging.path().join("manifest.json"),
        serde_json::to_vec(&Manifest {
            version: 1,
            browser: browser.is_some(),
        })?,
    )?;
    let destination = parent.join("vutnet");
    Ok(PreparedBundle {
        staging,
        destination,
    })
}

fn publish(staging: &Path, destination: &Path) -> Result<()> {
    let backup = staging.with_extension("previous");
    let previous = destination.exists();
    if previous {
        // Never replace/remove an unrelated directory or follow a symlink.
        if fs::symlink_metadata(destination)?.file_type().is_symlink() {
            bail!("bundle destination must not be a symlink");
        }
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(destination.join("manifest.json"))?)
                .context("existing directory is not a Vutnet bundle; refusing to overwrite it")?;
        if manifest.version != 1 {
            bail!("cannot replace an unsupported bundle version");
        }
        fs::rename(destination, &backup)
            .context("cannot move the previous bundle; stop the running server first")?;
    }
    if let Err(error) = fs::rename(staging, destination) {
        if previous {
            fs::rename(&backup, destination).context("cannot restore previous bundle")?;
        }
        return Err(error).context("cannot publish application bundle");
    }
    if previous && let Err(error) = fs::remove_dir_all(backup) {
        // Publication is already committed; cleanup must not roll CSS back.
        crate::ui::Ui::new().warning(&format!(
            "could not clean previous application bundle: {error}"
        ));
    }
    Ok(())
}

pub(crate) fn load(directory: &Path) -> Result<Bundle> {
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).with_context(|| {
            format!(
                "build bundle not found at {}; run vutnet build",
                directory.display()
            )
        })?)?;
    if manifest.version != 1 {
        bail!(
            "unsupported Vutnet build bundle version {}",
            manifest.version
        );
    }
    let artifact = directory.join(if cfg!(windows) { "app.exe" } else { "app" });
    if !artifact.is_file() {
        bail!("server executable missing from bundle; run vutnet build");
    }
    let browser = manifest.browser.then(|| directory.join("browser"));
    if let Some(browser) = &browser {
        for name in [
            "client.wasm",
            "client.js",
            "client.registry.json",
            "bootstrap.js",
            "navigation.js",
            "build-id",
        ] {
            if !browser.join(name).is_file() {
                bail!("browser asset {name} missing; run vutnet build");
            }
        }
    }
    Ok(Bundle { artifact, browser })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path, browser: bool) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join(if cfg!(windows) { "app.exe" } else { "app" }),
            b"server",
        )
        .unwrap();
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&Manifest {
                version: 1,
                browser,
            })
            .unwrap(),
        )
        .unwrap();
        if browser {
            fs::create_dir(root.join("browser")).unwrap();
            for name in [
                "client.js",
                "client.wasm",
                "client.registry.json",
                "bootstrap.js",
                "navigation.js",
                "build-id",
            ] {
                fs::write(root.join("browser").join(name), b"asset").unwrap();
            }
        }
    }
    #[test]
    fn complete_bundle_replaces_previous_build() {
        let temp = tempfile::tempdir().unwrap();
        let previous = temp.path().join("vutnet");
        fixture(&previous, false);
        let staged = temp.path().join("candidate");
        fixture(&staged, true);
        publish(&staged, &previous).unwrap();
        assert!(load(&previous).unwrap().browser.is_some());
        assert!(!staged.with_extension("previous").exists());
    }
    #[test]
    fn publication_refuses_unrelated_directories_and_repairs_missing_assets() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("vutnet");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("important"), b"keep").unwrap();
        let staged = temp.path().join("candidate");
        fixture(&staged, true);
        assert!(publish(&staged, &destination).is_err());
        assert_eq!(fs::read(destination.join("important")).unwrap(), b"keep");
        fs::remove_file(destination.join("important")).unwrap();
        fixture(&destination, true);
        fs::remove_file(destination.join("browser/client.wasm")).unwrap();
        publish(&staged, &destination).unwrap();
        assert!(load(&destination).is_ok());
    }

    #[test]
    fn invalid_and_incomplete_bundles_fail_cleanly() {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path(), true);
        fs::remove_file(temp.path().join("browser/client.wasm")).unwrap();
        assert!(
            load(temp.path())
                .unwrap_err()
                .to_string()
                .contains("client.wasm")
        );
    }
}
