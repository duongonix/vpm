//! Typed managed-import bridge for the Vut-authored `[[bin]]` dispatcher.

use vut_native_sdk::{AbiError, I32, Runtime, Str, Value};

fn exit_code(runtime: &Runtime, code: i32) -> Result<Value<I32>, AbiError> {
    runtime.copy_primitive::<I32>(code)
}

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_help_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::help())
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_version_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::version())
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_bad_args_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::invalid_arguments())
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_unknown_v1(command: Str) -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::unknown_command(command.as_str()?))
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_new_v1(name: Str) -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::new(name.as_str()?))
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_dev_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::dev(false))
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_dev_verbose_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::dev(true))
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_build_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::build())
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_run_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::run())
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_start_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::start())
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_routes_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::routes(false))
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_routes_json_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::routes(true))
    }
);

vut_native_sdk::managed_import!(
    pub fn vutnet_cli_info_v1() -> I32 {
        let runtime = Runtime::current()?;
        exit_code(&runtime, crate::cli::info())
    }
);
