use std::{env, fs, path::PathBuf};

pub(super) fn emit() {
    println!("cargo:rerun-if-changed=../vpm.toml");
    let native_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let manifest =
        fs::read_to_string(native_root.join("../vpm.toml")).expect("read Vutnet package manifest");
    let manifest: toml::Value = toml::from_str(&manifest).expect("parse Vutnet package manifest");
    let package_version = manifest["package"]["version"]
        .as_str()
        .expect("Vutnet package manifest has package.version");
    let native_version = env::var("CARGO_PKG_VERSION").expect("Cargo package version");
    assert_eq!(
        package_version, native_version,
        "vutnet-native version must match Vutnet vpm.toml package.version"
    );
    println!("cargo:rustc-env=VUTNET_VERSION={package_version}");
}
