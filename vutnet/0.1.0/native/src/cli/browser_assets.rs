//! Shared browser build output staging for dev generations and packaged apps.
use anyhow::{Context, Result, bail};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub(crate) fn stage(root: &Path, destination: &Path, release: bool) -> Result<Option<PathBuf>> {
    let profile = if release { "release" } else { "debug" };
    let source = root.join("target/wasm32-unknown-unknown").join(profile);
    if !source.join("vutnet-client.wasm").is_file() {
        return Ok(None);
    }
    let directory = destination.join("browser");
    fs::create_dir_all(&directory).context("cannot create browser asset directory")?;
    for extension in ["wasm", "js"] {
        let input = source.join(format!("vutnet-client.{extension}"));
        if !input.is_file() {
            bail!("VPM browser build did not produce {}", input.display());
        }
        fs::copy(&input, directory.join(format!("client.{extension}")))
            .with_context(|| format!("cannot stage browser artifact {}", input.display()))?;
    }
    let registry = source.join("vutnet-client.registry.json");
    if !registry.is_file() {
        bail!("VPM browser build did not produce {}", registry.display());
    }
    fs::copy(&registry, directory.join("client.registry.json"))
        .with_context(|| format!("cannot stage browser registry {}", registry.display()))?;
    let navigation_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/dev");
    for name in [
        "browser_navigation.js",
        "browser_navigation_forms.js",
        "browser_navigation_prefetch.js",
    ] {
        fs::copy(
            navigation_source.join(name),
            directory.join(name.replace("browser_", "")),
        )
        .with_context(|| format!("cannot stage browser navigation module {name}"))?;
    }
    let bootstrap = include_str!("commands/dev/browser_bootstrap.js");
    fs::write(directory.join("bootstrap.js"), bootstrap)
        .context("cannot stage browser bootstrap")?;
    let mut hasher = DefaultHasher::new();
    for name in [
        "client.js",
        "client.wasm",
        "client.registry.json",
        "bootstrap.js",
        "navigation.js",
        "navigation_forms.js",
        "navigation_prefetch.js",
    ] {
        fs::read(directory.join(name))?.hash(&mut hasher);
    }
    fs::write(
        directory.join("build-id"),
        format!("{:016x}", hasher.finish()),
    )
    .context("cannot write browser build identity")?;
    Ok(Some(directory))
}
