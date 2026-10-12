use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const CONFIG_FILE: &str = "vutnet.toml";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct VutnetConfig {
    pub tailwind: Option<TailwindConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TailwindConfig {
    pub version: String,
    pub input: PathBuf,
    pub output: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    tailwind: Option<TailwindFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TailwindFile {
    version: String,
    input: String,
    output: String,
}

impl VutnetConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(CONFIG_FILE);
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };
        let file: ConfigFile = toml::from_str(&source)
            .with_context(|| format!("invalid Vutnet configuration {}", path.display()))?;
        let tailwind = file
            .tailwind
            .map(TailwindFile::try_into)
            .transpose()
            .with_context(|| format!("invalid Vutnet configuration {}", path.display()))?;
        Ok(Self { tailwind })
    }
}

impl TryFrom<TailwindFile> for TailwindConfig {
    type Error = anyhow::Error;

    fn try_from(file: TailwindFile) -> Result<Self> {
        if !is_exact_version(&file.version) {
            bail!(
                "tailwind.version must be an exact numeric version such as `4.1.14`, got `{}`",
                file.version
            );
        }
        let input = project_relative_path("tailwind.input", &file.input)?;
        let output = project_relative_path("tailwind.output", &file.output)?;
        if input == output {
            bail!("tailwind.input and tailwind.output must be different paths");
        }
        Ok(Self {
            version: file.version,
            input,
            output,
        })
    }
}

fn project_relative_path(field: &str, value: &str) -> Result<PathBuf> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    let path = Path::new(value);
    if value.contains('\\')
        || has_windows_drive_prefix(value)
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("{field} must stay inside the project and be a relative path");
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        if let Component::Normal(part) = component {
            normalized.push(part);
        }
    }
    if normalized.as_os_str().is_empty() {
        bail!("{field} must name a project file");
    }
    Ok(normalized)
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_exact_version(version: &str) -> bool {
    let parts = version.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_disables_tailwind() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            VutnetConfig::load(temp.path()).unwrap(),
            VutnetConfig::default()
        );
    }

    #[test]
    fn parses_the_pinned_config_and_keeps_project_relative_paths() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(CONFIG_FILE),
            "[tailwind]\nversion = '4.1.14'\ninput = 'src/app.css'\noutput = 'public/assets/app.css'\n",
        )
        .unwrap();
        let config = VutnetConfig::load(temp.path()).unwrap();
        let tailwind = config.tailwind.unwrap();
        assert_eq!(tailwind.version, "4.1.14");
        assert_eq!(tailwind.input, PathBuf::from("src/app.css"));
        assert_eq!(tailwind.output, PathBuf::from("public/assets/app.css"));
        assert_eq!(
            temp.path().join(&tailwind.input),
            temp.path().join("src/app.css")
        );
    }

    #[test]
    fn rejects_unknown_keys_ranges_and_project_escape_paths() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(CONFIG_FILE);
        for source in [
            "[vutnet]\nmode='dev'\n",
            "[tailwind]\nversion='^4.1.14'\ninput='src/app.css'\noutput='public/app.css'\n",
            "[tailwind]\nversion='4.1.14'\ninput='../app.css'\noutput='public/app.css'\n",
            "[tailwind]\nversion='4.1.14'\ninput='C:/outside.css'\noutput='public/app.css'\n",
            "[tailwind]\nversion='4.1.14'\ninput='src\\\\app.css'\noutput='public/app.css'\n",
            "[tailwind]\nenabled=true\nversion='4.1.14'\ninput='src/app.css'\noutput='public/app.css'\n",
        ] {
            fs::write(&path, source).unwrap();
            let error = VutnetConfig::load(temp.path()).unwrap_err();
            assert!(error.to_string().contains(CONFIG_FILE));
        }
    }

    #[test]
    fn rejects_equal_input_and_output_paths() {
        let error: Result<TailwindConfig> = TailwindFile {
            version: "4.1.14".to_owned(),
            input: "src/app.css".to_owned(),
            output: "src/app.css".to_owned(),
        }
        .try_into();
        assert!(error.unwrap_err().to_string().contains("different paths"));
    }
}
