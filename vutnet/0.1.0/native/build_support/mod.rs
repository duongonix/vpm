mod generated;
mod types;
mod version;

use std::{env, fs, path::PathBuf};

pub fn run() {
    version::emit();
    println!("cargo:rerun-if-env-changed=VUT_PACKAGE_IDENTITY");
    println!("cargo:rerun-if-env-changed=VUT_ABI_SCHEMA_MANIFEST");
    let package_identity = env::var("VUT_PACKAGE_IDENTITY").unwrap_or_else(|_| "vutnet".into());
    let manifest_path = env::var_os("VUT_ABI_SCHEMA_MANIFEST")
        .map(PathBuf::from)
        .expect("VPM must provide the compiler-generated VUT_ABI_SCHEMA_MANIFEST");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let manifest = fs::read_to_string(&manifest_path)
        .expect("read compiler-generated Vut ABI schema manifest");
    let identities = types::VutnetAbi::from_manifest(&package_identity, &manifest);
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    generated::write(&output, &identities).expect("write generated ABI identities");
}
