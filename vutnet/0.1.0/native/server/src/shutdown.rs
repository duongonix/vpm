use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

/// Shared, race-safe shutdown signal.
///
/// `wait` registers a waiter before re-checking the flag, so a `request` that
/// races with it is never lost.
#[derive(Debug, Default)]
pub(crate) struct ShutdownState {
    requested: AtomicBool,
    notify: Notify,
}

impl ShutdownState {
    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    pub(crate) fn request(&self) {
        if !self.requested.swap(true, Ordering::AcqRel) {
            self.notify.notify_waiters();
        }
    }

    pub(crate) async fn wait(&self) {
        loop {
            if self.is_requested() {
                return;
            }
            let notified = self.notify.notified();
            if self.is_requested() {
                return;
            }
            notified.await;
        }
    }
}

/// Handle used to request clean shutdown of a running [`crate::Server`].
///
/// Triggering shutdown stops the accept loop, asks every in-flight connection
/// to finish its current request and close, and resolves the server future once
/// all connections are gone.
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    state: Arc<ShutdownState>,
}

impl ShutdownHandle {
    /// Creates an un-triggered shutdown handle.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(ShutdownState::default()),
        }
    }

    /// Whether shutdown has already been requested.
    #[must_use]
    pub fn is_requested(&self) -> bool {
        self.state.is_requested()
    }

    /// Requests shutdown. Repeated calls are idempotent.
    pub fn shutdown(&self) {
        self.state.request();
    }

    pub(crate) fn state(&self) -> Arc<ShutdownState> {
        Arc::clone(&self.state)
    }
}

impl Default for ShutdownHandle {
    fn default() -> Self {
        Self::new()
    }
}
