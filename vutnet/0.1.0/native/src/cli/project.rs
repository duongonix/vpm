use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    manifest: toml::Value,
}

impl Project {
    pub fn discover(cwd: &Path) -> Result<Self> {
        for root in cwd.ancestors() {
            let path = root.join("vpm.toml");
            if !path.is_file() {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let manifest: toml::Value = toml::from_str(&source)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            let package = manifest
                .get("package")
                .and_then(toml::Value::as_table)
                .ok_or_else(|| anyhow!("{} has no [package] table", path.display()))?;
            for field in ["name", "version"] {
                if package.get(field).and_then(toml::Value::as_str).is_none() {
                    bail!("{} is missing package.{field}", path.display());
                }
            }
            return Ok(Self {
                root: root.to_path_buf(),
                manifest,
            });
        }
        bail!("no VPM project found; run this command from a directory with vpm.toml")
    }

    pub fn package_name(&self) -> &str {
        self.manifest["package"]["name"]
            .as_str()
            .expect("Project::discover validates package.name")
    }

    pub fn vutnet_dependency_version(&self) -> Option<&str> {
        let dependency = self.manifest.get("dependencies")?.get("vutnet")?;
        dependency
            .as_str()
            .or_else(|| dependency.get("version")?.as_str())
    }

    pub fn release_artifact(&self) -> PathBuf {
        artifact_path(&self.root, "release")
    }

    pub fn entry_path(&self) -> PathBuf {
        let main = self.root.join("src/main.vut");
        if main.is_file() {
            return main;
        }
        self.manifest
            .get("bin")
            .and_then(toml::Value::as_array)
            .and_then(|bins| {
                bins.iter().find_map(|bin| {
                    let kind = bin.get("kind").and_then(toml::Value::as_str);
                    let path = bin.get("path").and_then(toml::Value::as_str)?;
                    (kind.is_none() || kind == Some("vut")).then(|| self.root.join(path))
                })
            })
            .unwrap_or(main)
    }

    pub fn local_dependency_roots(&self) -> Result<Vec<PathBuf>> {
        let mut pending = self.local_dependencies(&self.root);
        let mut roots = Vec::new();
        while let Some(path) = pending.pop() {
            let canonical = path.canonicalize().with_context(|| {
                format!("local dependency path does not exist: {}", path.display())
            })?;
            if roots.contains(&canonical) {
                continue;
            }
            let manifest_path = canonical.join("vpm.toml");
            let source = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("failed to read {}", manifest_path.display()))?;
            let manifest: toml::Value = toml::from_str(&source)
                .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
            pending.extend(local_dependency_paths(&manifest, &canonical));
            roots.push(canonical);
        }
        Ok(roots)
    }

    fn local_dependencies(&self, root: &Path) -> Vec<PathBuf> {
        local_dependency_paths(&self.manifest, root)
    }
}

fn local_dependency_paths(manifest: &toml::Value, root: &Path) -> Vec<PathBuf> {
    manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::map::Map::values)
        .filter_map(|dependency| dependency.get("path").and_then(toml::Value::as_str))
        .map(|path| root.join(path))
        .collect()
}

pub fn artifact_path(root: &Path, profile: &str) -> PathBuf {
    root.join("build")
        .join(profile)
        .join(if cfg!(windows) { "app.exe" } else { "app" })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn discovers_project_ancestor_and_conventional_entry() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("src/nested")).unwrap();
        fs::write(
            temp.path().join("vpm.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n[dependencies]\n",
        )
        .unwrap();
        fs::write(temp.path().join("src/main.vut"), "").unwrap();
        let project = Project::discover(&temp.path().join("src/nested")).unwrap();
        assert_eq!(project.root, temp.path());
        assert_eq!(project.entry_path(), temp.path().join("src/main.vut"));
        assert_eq!(
            artifact_path(temp.path(), "debug"),
            temp.path()
                .join("build/debug")
                .join(if cfg!(windows) { "app.exe" } else { "app" })
        );
    }

    #[test]
    fn reads_registry_and_vut_bin_project_manifests() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("vpm.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n[dependencies]\nvutnet='0.1.0'\n\n[[bin]]\nname='tool'\npath='src/bin/tool.vut'\n",
        )
        .unwrap();
        let project = Project::discover(temp.path()).unwrap();
        assert_eq!(project.package_name(), "demo");
        assert_eq!(project.vutnet_dependency_version(), Some("0.1.0"));
        assert_eq!(project.entry_path(), temp.path().join("src/bin/tool.vut"));
    }

    #[test]
    fn follows_transitive_local_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("app");
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(
            root.join("vpm.toml"),
            "[package]\nname='app'\nversion='0.1.0'\n[dependencies]\nfirst={path='../first'}\n",
        )
        .unwrap();
        fs::write(
            first.join("vpm.toml"),
            "[package]\nname='first'\nversion='0.1.0'\n[dependencies]\nsecond={path='../second'}\n",
        )
        .unwrap();
        fs::write(
            second.join("vpm.toml"),
            "[package]\nname='second'\nversion='0.1.0'\n",
        )
        .unwrap();
        let project = Project::discover(&root).unwrap();
        let roots = project.local_dependency_roots().unwrap();
        assert!(roots.contains(&first.canonicalize().unwrap()));
        assert!(roots.contains(&second.canonicalize().unwrap()));
    }

    #[test]
    fn reports_missing_project() {
        let temp = tempfile::tempdir().unwrap();
        assert!(Project::discover(temp.path()).is_err());
    }
}
