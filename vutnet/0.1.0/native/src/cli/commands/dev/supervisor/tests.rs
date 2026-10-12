use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, OnceLock, atomic::AtomicU64},
};

use anyhow::{Result, bail};

use super::*;

#[derive(Clone)]
enum BuildStep {
    Success,
    Failure(&'static str),
    Cancelled,
    DirtySuccess,
}

struct FakeProcess(u64);

struct FakeRuntime {
    builds: VecDeque<BuildStep>,
    spawn_failures: VecDeque<bool>,
    exits: VecDeque<ServerExit>,
    events: Vec<String>,
    active: HashSet<u64>,
    max_active: usize,
    next_process: u64,
    revision: Arc<AtomicU64>,
}

impl FakeRuntime {
    fn new(revision: Arc<AtomicU64>, builds: impl IntoIterator<Item = BuildStep>) -> Self {
        Self {
            builds: builds.into_iter().collect(),
            spawn_failures: VecDeque::new(),
            exits: VecDeque::new(),
            events: Vec::new(),
            active: HashSet::new(),
            max_active: 0,
            next_process: 1,
            revision,
        }
    }
}

impl DevRuntime for FakeRuntime {
    type Process = FakeProcess;

    fn build_candidate(&mut self, stopping: &AtomicBool) -> Result<Option<CandidateArtifact>> {
        self.events.push("build".to_owned());
        if stopping.load(Ordering::Acquire) {
            return Ok(None);
        }
        let generation = self.events.iter().filter(|event| *event == "build").count() as u64;
        match self.builds.pop_front().unwrap_or(BuildStep::Success) {
            BuildStep::Success => Ok(Some(CandidateArtifact::for_test(
                PathBuf::from(format!("candidate-{generation}")),
                generation,
            ))),
            BuildStep::Failure(message) => {
                bail!("{message}");
            }
            BuildStep::Cancelled => Ok(None),
            BuildStep::DirtySuccess => {
                self.revision.fetch_add(1, Ordering::Release);
                Ok(Some(CandidateArtifact::for_test(
                    PathBuf::from(format!("candidate-{generation}")),
                    generation,
                )))
            }
        }
    }

    fn spawn_server(&mut self, artifact: &CandidateArtifact) -> Result<Self::Process> {
        self.events.push(format!("spawn:{}", artifact.generation()));
        if self.spawn_failures.pop_front().unwrap_or(false) {
            bail!("injected server spawn failure");
        }
        let id = self.next_process;
        self.next_process += 1;
        self.active.insert(id);
        self.max_active = self.max_active.max(self.active.len());
        Ok(FakeProcess(id))
    }

    fn try_wait(&mut self, process: &mut Self::Process) -> Result<Option<ServerExit>> {
        if let Some(exit) = self.exits.pop_front() {
            self.active.remove(&process.0);
            self.events.push(format!("exit:{}", process.0));
            return Ok(Some(exit));
        }
        Ok(None)
    }

    fn stop(&mut self, process: &mut Self::Process) -> Result<()> {
        self.events.push(format!("stop:{}", process.0));
        self.active.remove(&process.0);
        Ok(())
    }

    fn cleanup_artifact(&mut self, artifact: &CandidateArtifact) -> Result<()> {
        self.events
            .push(format!("cleanup:{}", artifact.generation()));
        Ok(())
    }

    fn publish_assets(&mut self, artifact: &mut CandidateArtifact) -> Result<()> {
        self.events
            .push(format!("publish:{}", artifact.generation()));
        Ok(())
    }

    fn rollback_assets(&mut self, artifact: &mut CandidateArtifact) -> Result<()> {
        self.events
            .push(format!("rollback:{}", artifact.generation()));
        Ok(())
    }

    fn finish_assets(&mut self, artifact: &mut CandidateArtifact) -> Result<()> {
        self.events
            .push(format!("finish:{}", artifact.generation()));
        Ok(())
    }
}

fn supervisor(runtime: FakeRuntime) -> DevSupervisor<'static, FakeRuntime> {
    static TEST_UI: OnceLock<Ui> = OnceLock::new();
    DevSupervisor::new(runtime, TEST_UI.get_or_init(Ui::new))
}

fn current_revision(value: &Arc<AtomicU64>) -> impl Fn() -> u64 + '_ {
    move || value.load(Ordering::Acquire)
}

fn rebuild(supervisor: &mut DevSupervisor<'_, FakeRuntime>, revision: &Arc<AtomicU64>) {
    supervisor.rebuild(current_revision(revision), &AtomicBool::new(false));
}

#[test]
fn initial_build_success_starts_only_one_server() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(revision.clone(), [BuildStep::Success]);
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);

    assert!(supervisor.server.is_some());
    assert_eq!(supervisor.runtime.active.len(), 1);
    assert_eq!(supervisor.runtime.max_active, 1);
}

#[test]
fn initial_compile_failures_keep_supervisor_alive_and_later_build_recovers() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(
        revision.clone(),
        [
            BuildStep::Failure("first compile error"),
            BuildStep::Failure("second compile error"),
            BuildStep::Success,
        ],
    );
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    assert!(supervisor.server.is_none());
    rebuild(&mut supervisor, &revision);
    assert!(supervisor.server.is_none());
    rebuild(&mut supervisor, &revision);
    assert!(supervisor.server.is_some());
    assert_eq!(supervisor.runtime.active.len(), 1);
}

#[test]
fn successful_candidate_stops_old_process_before_replacement_binds() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(revision.clone(), [BuildStep::Success, BuildStep::Success]);
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    rebuild(&mut supervisor, &revision);

    assert_eq!(supervisor.runtime.max_active, 1);
    let events = &supervisor.runtime.events;
    let stop = events.iter().position(|event| event == "stop:1").unwrap();
    let spawn = events.iter().position(|event| event == "spawn:2").unwrap();
    assert!(stop < spawn, "events: {events:?}");
    let publish = events
        .iter()
        .position(|event| event == "publish:2")
        .unwrap();
    assert!(stop < publish && publish < spawn, "events: {events:?}");
}

#[test]
fn compile_failures_leave_the_current_server_running_until_success() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(
        revision.clone(),
        [
            BuildStep::Success,
            BuildStep::Failure("broken edit"),
            BuildStep::Failure("another broken edit"),
            BuildStep::Success,
        ],
    );
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    rebuild(&mut supervisor, &revision);
    rebuild(&mut supervisor, &revision);
    assert_eq!(supervisor.runtime.active.len(), 1);
    assert_eq!(supervisor.runtime.max_active, 1);
    assert_eq!(supervisor.server.as_ref().unwrap().process.0, 1);
    rebuild(&mut supervisor, &revision);
    assert_eq!(supervisor.runtime.active.len(), 1);
    assert_eq!(supervisor.server.as_ref().unwrap().process.0, 2);
}

#[test]
fn changes_during_compile_discard_stale_candidates_without_replacing_server() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(
        revision.clone(),
        [
            BuildStep::Success,
            BuildStep::DirtySuccess,
            BuildStep::Success,
        ],
    );
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    rebuild(&mut supervisor, &revision);

    assert_eq!(supervisor.server.as_ref().unwrap().process.0, 1);
    assert_eq!(supervisor.runtime.active.len(), 1);
    assert!(supervisor.runtime.events.contains(&"cleanup:2".to_owned()));

    rebuild(&mut supervisor, &revision);
    assert_eq!(supervisor.server.as_ref().unwrap().process.0, 2);
    assert_eq!(supervisor.runtime.max_active, 1);
}

#[test]
fn spawn_failure_rolls_back_assets_and_restarts_last_successful_server() {
    let revision = Arc::new(AtomicU64::new(0));
    let mut runtime = FakeRuntime::new(
        revision.clone(),
        [BuildStep::Success, BuildStep::Success, BuildStep::Success],
    );
    runtime.spawn_failures = [false, true, false].into();
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    rebuild(&mut supervisor, &revision);
    assert_eq!(supervisor.server.as_ref().unwrap().process.0, 2);
    assert_eq!(supervisor.runtime.active.len(), 1);
    assert_eq!(supervisor.last_successful.as_ref().unwrap().generation(), 1);
    assert!(supervisor.runtime.events.contains(&"rollback:2".to_owned()));
    rebuild(&mut supervisor, &revision);
    assert_eq!(supervisor.server.as_ref().unwrap().process.0, 3);
    assert_eq!(supervisor.runtime.max_active, 1);
}

#[test]
fn runtime_crash_leaves_supervisor_waiting_for_a_save() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(revision.clone(), [BuildStep::Success, BuildStep::Success]);
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    supervisor.runtime.exits.push_back(ServerExit {
        success: false,
        display: "exit code 1".to_owned(),
    });
    supervisor.poll_server().unwrap();
    assert!(supervisor.server.is_none());
    assert_eq!(supervisor.runtime.active.len(), 0);
    assert_eq!(
        supervisor
            .runtime
            .events
            .iter()
            .filter(|event| event.starts_with("spawn:"))
            .count(),
        1,
        "a crash does not cause an automatic restart"
    );

    rebuild(&mut supervisor, &revision);
    assert!(supervisor.server.is_some());
}

#[test]
fn ctrl_c_shutdown_stops_server_and_cleans_owned_generation() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(revision.clone(), [BuildStep::Success]);
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);
    supervisor.shutdown().unwrap();

    assert!(supervisor.server.is_none());
    assert_eq!(supervisor.runtime.active.len(), 0);
    assert!(supervisor.runtime.events.contains(&"stop:1".to_owned()));
    assert!(supervisor.runtime.events.contains(&"cleanup:1".to_owned()));
}

#[test]
fn cancelled_candidate_is_not_started() {
    let revision = Arc::new(AtomicU64::new(0));
    let runtime = FakeRuntime::new(revision.clone(), [BuildStep::Cancelled]);
    let mut supervisor = supervisor(runtime);
    rebuild(&mut supervisor, &revision);

    assert!(supervisor.server.is_none());
    assert!(supervisor.runtime.active.is_empty());
}
