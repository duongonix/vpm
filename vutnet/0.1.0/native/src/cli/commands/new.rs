use std::{fs, path::Path};

use anyhow::{Result, bail};

use crate::{toolchain, ui::Ui};

const VUTNET_VERSION: &str = env!("VUTNET_VERSION");
const VUTNET_CONFIG: &str = r"# Vutnet project configuration.
# Add [tailwind] to enable Tailwind CSS.
";

pub(super) fn run(name: &str, ui: &Ui) -> Result<()> {
    let parent = std::env::current_dir()?;
    let root = parent.join(name);
    ensure_destination_available(&root)?;
    toolchain::create_project(&parent, name)?;
    write_scaffold(&root, name)?;
    ui.header();
    if let Err(error) = toolchain::update_project(&root) {
        ui.warning("project scaffold was created, but dependency resolution did not complete");
        bail!(
            "dependency resolution failed: {error:#}\nrun `vpm update` after checking registry access"
        );
    }
    ui.status(&format!("Created {}", root.display()));
    ui.detail("Next", &format!("cd {name}; vutnet dev"));
    Ok(())
}

fn ensure_destination_available(root: &Path) -> Result<()> {
    if root.exists() {
        bail!("destination already exists: {}", root.display());
    }
    Ok(())
}

fn write_scaffold(root: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(root.join("src/controllers"))?;
    fs::create_dir_all(root.join("public"))?;
    fs::write(root.join("public/.keep"), "")?;
    fs::write(root.join("src/main.vut"), MAIN)?;
    fs::write(root.join("src/router.vut"), ROUTER)?;
    fs::write(root.join("src/controllers/home.vut"), HOME)?;
    fs::write(
        root.join("vpm.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\nvutnet = \"{VUTNET_VERSION}\"\n"
        ),
    )?;
    fs::write(root.join("vutnet.toml"), VUTNET_CONFIG)?;
    fs::write(root.join(".gitignore"), "/build/\n/.vpm/\n/.vut/cache/\n")?;
    fs::write(root.join("README.md"), README.replace("{{name}}", name))?;
    Ok(())
}

const MAIN: &str = "import vutnet\nimport router\n\nfn main() -> int:\n  app = vutnet.app()\n  app.router(router.RootRouter)\n  app.listen(3000)\n  0\n";

const ROUTER: &str = "import vutnet.router at router, Router\nimport controllers.home at HomeController\n\nfn RootRouter() -> Router:\n  router:\n    Route(path = \"/\", controller = HomeController)\n";

const HOME: &str = "import vutnet.context at Context\nimport vutnet.response at Response\nimport vutnet.response as response\n\nfn HomeController(ctx: Context) -> Response:\n  response.send(200, \"Hello from Vutnet\")\n";

const README: &str = "# {{name}}\n\nA Vutnet application.\n\n## Development\n\n```sh\nvutnet dev\n```\n\n## Production\n\n```sh\nvutnet build\nvutnet start\n```\n";

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn scaffold_uses_current_vutnet_syntax_and_package_version() {
        let temp = tempfile::tempdir().unwrap();
        write_scaffold(temp.path(), "sample").unwrap();
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(temp.path().join("vpm.toml")).unwrap()).unwrap();
        assert_eq!(
            manifest["dependencies"]["vutnet"].as_str(),
            Some(VUTNET_VERSION)
        );
        let config_path = temp.path().join("vutnet.toml");
        let config_source = fs::read_to_string(config_path).unwrap();
        let config: toml::Value = toml::from_str(&config_source).unwrap();
        assert!(config.get("tailwind").is_none());
        assert!(temp.path().join("src/router.vut").is_file());
        assert!(temp.path().join("src/controllers/home.vut").is_file());
        let router = fs::read_to_string(temp.path().join("src/router.vut")).unwrap();
        assert!(router.contains("Route(path = \"/\", controller = HomeController)"));
        assert!(!router.contains("path: \"/\""));
    }

    #[test]
    fn existing_destination_is_not_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("existing");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("keep.txt"), "keep").unwrap();
        assert!(ensure_destination_available(&root).is_err());
        assert_eq!(fs::read_to_string(root.join("keep.txt")).unwrap(), "keep");
    }
}
