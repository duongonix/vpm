use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};

const DEBOUNCE: Duration = Duration::from_millis(120);

pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
    root: PathBuf,
    watched: Vec<PathBuf>,
    ignored: Vec<PathBuf>,
    css_enabled: bool,
    change_revision: Arc<AtomicU64>,
}

impl FileWatcher {
    pub fn new(root: &Path, local_dependencies: &[PathBuf]) -> Result<Self> {
        Self::new_with_tailwind(root, local_dependencies, None)
    }

    pub fn new_with_tailwind(
        root: &Path,
        local_dependencies: &[PathBuf],
        css_output: Option<&Path>,
    ) -> Result<Self> {
        let css_enabled = css_output.is_some();
        let root = root.canonicalize()?;
        let watched = watched_paths(&root, local_dependencies, css_enabled)?;
        let ignored = ignored_paths(&root, local_dependencies, css_enabled, css_output);

        let (sender, events) = mpsc::channel();
        let change_revision = Arc::new(AtomicU64::new(0));
        let event_revision = Arc::clone(&change_revision);
        let callback_watched = watched.clone();
        let callback_ignored = ignored.clone();
        let mut file_watcher = notify::recommended_watcher(move |event| {
            if let Ok(event) = &event
                && has_relevant_paths(event, &callback_watched, &callback_ignored, css_enabled)
            {
                event_revision.fetch_add(1, Ordering::Release);
            }
            let _ = sender.send(event);
        })?;
        for path in &watched {
            file_watcher.watch(
                path,
                if path.is_dir() {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )?;
        }
        Ok(Self {
            _watcher: file_watcher,
            events,
            root,
            watched,
            ignored,
            css_enabled,
            change_revision,
        })
    }

    /// Monotonically increases as relevant source events arrive, including while
    /// the supervisor is blocked compiling a candidate.
    pub fn change_revision(&self) -> u64 {
        self.change_revision.load(Ordering::Acquire)
    }

    pub fn project_root(&self) -> &Path {
        &self.root
    }

    /// Returns one relevant path after a burst of OS events has settled.
    pub fn next_change(&self, timeout: Duration) -> Result<Option<PathBuf>> {
        let first = match self.events.recv_timeout(timeout) {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => return Err(error.into()),
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                anyhow::bail!("filesystem watcher event stream closed");
            }
        };
        let mut paths = BTreeSet::new();
        self.collect(&first, &mut paths);
        let mut deadline = Instant::now() + DEBOUNCE;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self
                .events
                .recv_timeout(deadline.saturating_duration_since(now))
            {
                Ok(Ok(event)) => {
                    if self.collect(&event, &mut paths) {
                        deadline = Instant::now() + DEBOUNCE;
                    }
                }
                Ok(Err(error)) => return Err(error.into()),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("filesystem watcher event stream closed");
                }
            }
        }
        Ok(paths.into_iter().next())
    }

    fn collect(&self, event: &Event, selected: &mut BTreeSet<PathBuf>) -> bool {
        collect_relevant(
            event,
            &self.watched,
            &self.ignored,
            self.css_enabled,
            selected,
        )
    }
}

fn watched_paths(
    root: &Path,
    local_dependencies: &[PathBuf],
    css_enabled: bool,
) -> Result<Vec<PathBuf>> {
    let mut watched = Vec::new();
    if css_enabled {
        watched.push(root.to_path_buf());
    } else {
        let source = root.join("src");
        if source.is_dir() {
            watched.push(source);
        }
        for file in ["vpm.toml", "vpm.lock"] {
            let path = root.join(file);
            if path.is_file() {
                watched.push(path);
            }
        }
    }
    watched.extend(
        local_dependencies
            .iter()
            .filter(|dependency| dependency.is_dir())
            .cloned(),
    );
    watched.sort();
    watched.dedup();
    watched
        .into_iter()
        .map(|path| {
            path.canonicalize()
                .with_context(|| format!("cannot watch {}", path.display()))
        })
        .collect()
}

fn ignored_paths(
    root: &Path,
    local_dependencies: &[PathBuf],
    css_enabled: bool,
    css_output: Option<&Path>,
) -> Vec<PathBuf> {
    let mut ignored = [
        "build",
        "target",
        ".vpm",
        ".vut",
        ".git",
        "node_modules",
        "dist",
        "vendor",
        ".idea",
    ]
    .into_iter()
    .map(|path| root.join(path))
    .collect::<Vec<_>>();
    if css_enabled {
        ignored.push(root.join("vutnet.toml"));
    }
    if let Some(output) = css_output {
        ignored.push(output.to_path_buf());
    }
    ignored.extend(local_dependencies.iter().flat_map(|dependency| {
        [
            "build",
            "target",
            ".vpm",
            ".vut",
            ".git",
            "node_modules",
            "dist",
            "vendor",
        ]
        .map(|part| dependency.join(part))
    }));
    ignored.into_iter().map(normalize).collect()
}

fn has_relevant_paths(
    event: &Event,
    watched: &[PathBuf],
    ignored: &[PathBuf],
    css_enabled: bool,
) -> bool {
    let mut selected = BTreeSet::new();
    collect_relevant(event, watched, ignored, css_enabled, &mut selected)
}

fn collect_relevant(
    event: &Event,
    watched: &[PathBuf],
    ignored: &[PathBuf],
    css_enabled: bool,
    selected: &mut BTreeSet<PathBuf>,
) -> bool {
    if !matches!(
        &event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    let mut changed = false;
    for path in &event.paths {
        let path = normalize(path.clone());
        if ignored
            .iter()
            .any(|ignored| path == *ignored || path.starts_with(ignored))
        {
            continue;
        }
        if !watched
            .iter()
            .any(|root| path == *root || path.starts_with(root))
        {
            continue;
        }
        if !supported_source(&path, css_enabled) {
            continue;
        }
        selected.insert(path);
        changed = true;
    }
    changed
}

fn normalize(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn supported_source(path: &Path, css_enabled: bool) -> bool {
    if matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("vpm.toml" | "vpm.lock")
    ) {
        return true;
    }
    let extension = path.extension().and_then(|extension| extension.to_str());
    matches!(
        extension.map(str::to_ascii_lowercase).as_deref(),
        Some("vut" | "toml" | "lock" | "rs" | "c" | "h" | "cc" | "cpp")
    ) || (css_enabled && extension.is_some_and(|extension| extension.eq_ignore_ascii_case("css")))
}

#[cfg(test)]
mod tests {
    use std::{fs, thread};

    use super::*;

    #[test]
    fn detects_source_events_and_ignores_build_output() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("src")).unwrap();
        fs::create_dir_all(temp.path().join("build/debug")).unwrap();
        fs::write(temp.path().join("src/main.vut"), "fn main(): 0").unwrap();
        let watcher = FileWatcher::new(temp.path(), &[]).unwrap();
        fs::write(temp.path().join("build/debug/generated.vut"), "ignored").unwrap();
        fs::write(temp.path().join("src/main.vut"), "fn main(): 1").unwrap();
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            watcher.next_change(Duration::from_secs(2)).unwrap(),
            Some(temp.path().join("src/main.vut").canonicalize().unwrap())
        );
    }

    #[test]
    fn filters_non_source_asset_changes() {
        assert!(!supported_source(Path::new("src/site.css"), false));
        assert!(supported_source(Path::new("src/site.css"), true));
        assert!(supported_source(Path::new("src/page.vut"), false));
        assert!(supported_source(Path::new("vpm.toml"), false));
    }

    #[test]
    fn coalesces_source_bursts_and_ignores_dev_generation_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("src/main.vut");
        let generated = root.join(".vut/dev/generated.vut");
        let watched = [root.clone()];
        let ignored = [root.join(".vut/dev")];
        let mut selected = BTreeSet::new();
        let event = Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
            .add_path(source.clone())
            .add_path(source.clone())
            .add_path(generated);

        assert!(collect_relevant(
            &event,
            &watched,
            &ignored,
            false,
            &mut selected
        ));
        assert_eq!(selected.into_iter().collect::<Vec<_>>(), vec![source]);
    }

    #[test]
    fn tailwind_watches_css_and_vutnet_changes_but_ignores_generated_css_and_config() {
        let root = Path::new("C:/work/app");
        let watched = [root.to_path_buf()];
        let ignored = [
            root.join(".vut"),
            root.join("vutnet.toml"),
            root.join("public/assets/app.css"),
        ];
        let css_source = root.join("styles/base.css");
        let generated_css = root.join("public/assets/app.css");
        let config = root.join("vutnet.toml");
        let event = Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
            .add_path(css_source.clone())
            .add_path(generated_css)
            .add_path(config);
        let mut selected = BTreeSet::new();

        assert!(collect_relevant(
            &event,
            &watched,
            &ignored,
            true,
            &mut selected
        ));
        assert_eq!(selected.into_iter().collect::<Vec<_>>(), vec![css_source]);
    }
}
