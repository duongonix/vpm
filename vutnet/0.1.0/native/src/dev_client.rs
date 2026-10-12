//! Browser client asset contract shared by dev and packaged applications.

use std::path::PathBuf;

pub(crate) const ASSET_DIRECTORY_ENV: &str = "VUTNET_BROWSER_DIR";
pub(crate) const ASSET_MOUNT_PREFIX: &str = "/__vutnet";

pub(crate) fn asset_directory() -> Option<PathBuf> {
    std::env::var_os(ASSET_DIRECTORY_ENV).map(PathBuf::from)
}

pub(crate) fn asset_version() -> Option<String> {
    let source = asset_directory()?.join("build-id");
    let version = std::fs::read_to_string(source).ok()?;
    let version = version.trim();
    (version.len() == 16 && version.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| version.to_ascii_lowercase())
}

pub(crate) fn enabled() -> bool {
    asset_directory().is_some()
}
