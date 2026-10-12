use std::{fmt::Write as _, fs, io, path::Path};

use super::types::VutnetAbi;

pub fn write(output_dir: &Path, abi: &VutnetAbi) -> io::Result<()> {
    let mut source = String::from("use vut_native_abi::VutTypeIdV1;\n");
    for nominal in &abi.nominals {
        let high = hex_u64(nominal.type_id.high);
        let low = hex_u64(nominal.type_id.low);
        let _ = writeln!(
            source,
            "pub const {}: VutTypeIdV1 = VutTypeIdV1 {{ high: {high}, low: {low} }};",
            nominal.constant
        );
        let _ = writeln!(
            source,
            "pub const {}_SCHEMA_VERSION: u32 = {};",
            nominal.constant, nominal.schema_version
        );
    }
    let _ = writeln!(
        source,
        "pub const CONTROLLER_SIGNATURE: VutTypeIdV1 = VutTypeIdV1 {{ high: {}, low: {} }};",
        hex_u64(abi.controller_signature.high),
        hex_u64(abi.controller_signature.low)
    );
    fs::write(output_dir.join("type_ids.rs"), source)
}

fn hex_u64(value: u64) -> String {
    format!(
        "0x{:04x}_{:04x}_{:04x}_{:04x}",
        (value >> 48) & 0xffff,
        (value >> 32) & 0xffff,
        (value >> 16) & 0xffff,
        value & 0xffff,
    )
}
