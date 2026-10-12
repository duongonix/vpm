use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::Notify;

/// Counts live connection tasks so shutdown can wait for them without a join
/// set. The counter is decremented by an RAII guard, so a panicking or
/// cancelled connection still releases its slot.
#[derive(Debug, Default)]
pub(crate) struct ConnectionTracker {
    active: AtomicUsize,
    idle: Notify,
}

impl ConnectionTracker {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn track(self: &Arc<Self>) -> ConnectionGuard {
        self.active.fetch_add(1, Ordering::AcqRel);
        ConnectionGuard {
            tracker: Arc::clone(self),
        }
    }

    pub(crate) async fn wait_idle(&self) {
        loop {
            if self.active.load(Ordering::Acquire) == 0 {
                return;
            }
            let notified = self.idle.notified();
            if self.active.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConnectionGuard {
    tracker: Arc<ConnectionTracker>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if self.tracker.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.tracker.idle.notify_waiters();
        }
    }
}
