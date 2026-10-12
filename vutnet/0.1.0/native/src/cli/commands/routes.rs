use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    project::Project,
    toolchain,
    ui::{RouteRow, Ui},
};

#[derive(Debug, Deserialize, Serialize)]
struct RouteDump {
    routes: Vec<RouteEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RouteEntry {
    path: String,
    methods: Vec<String>,
}

pub(super) fn run(json: bool, ui: &Ui) -> Result<()> {
    let project = Project::discover(&std::env::current_dir()?)?;
    let artifact = toolchain::build_project(&project, false)?;
    let output = Command::new(&artifact)
        .current_dir(&project.root)
        .env("VUTNET_CLI_ROUTE_DUMP", "1")
        .output()
        .context("failed to execute application route discovery")?;
    if !output.status.success() {
        bail!(
            "route discovery failed:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut dump: RouteDump = serde_json::from_slice(&output.stdout)
        .context("application did not return the Vutnet route table")?;
    dump.routes
        .sort_by(|left, right| left.path.cmp(&right.path));
    for route in &mut dump.routes {
        route.methods.sort();
    }
    if json {
        Ui::stdout(&serde_json::to_string_pretty(&dump)?);
        Ui::stdout("\n");
        return Ok(());
    }
    let rows = dump
        .routes
        .iter()
        .flat_map(|route| {
            route.methods.iter().map(|method| RouteRow {
                method: method.clone(),
                path: route.path.clone(),
            })
        })
        .collect::<Vec<_>>();
    ui.header();
    ui.heading("VUTNET ROUTES");
    ui.route_table(&rows);
    ui.dim(&format!("{} routes", rows.len()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_dump_shape_is_machine_readable() {
        let dump: RouteDump = serde_json::from_str(
            r#"{"routes":[{"path":"/admin/users","methods":["GET","POST"]}]}"#,
        )
        .unwrap();
        assert_eq!(dump.routes[0].path, "/admin/users");
        assert_eq!(dump.routes[0].methods, ["GET", "POST"]);
    }
}
