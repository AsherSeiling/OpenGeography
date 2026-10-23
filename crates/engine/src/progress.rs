//! Progress of long-running work (downloads, imports, model runs), polled by
//! the UI to draw progress bars. A task lives as long as its [`Task`] handle.

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub id: u64,
    pub label: String,
    /// 0..=1, or None while the total is unknown.
    pub fraction: Option<f64>,
    pub detail: String,
    /// The work checks for cancellation, so the UI may offer Cancel.
    pub cancellable: bool,
    #[serde(skip)]
    cancel_requested: bool,
}

thread_local! {
    /// The cancel flag of the model run executing on this thread, if any.
    static RUN_CANCEL: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` with `flag` as this thread's cancel flag: GDAL raster steps it
/// starts stop when the flag is set (see `raster::gdal_progress`).
pub fn with_cancel_flag<R>(flag: std::sync::Arc<std::sync::atomic::AtomicBool>, f: impl FnOnce() -> R) -> R {
    let prev = RUN_CANCEL.with(|c| c.replace(Some(flag)));
    let out = f();
    RUN_CANCEL.with(|c| *c.borrow_mut() = prev);
    out
}

/// The `pProgressArg` for GDAL calls: this thread's run cancel flag, or null
/// (never cancelled) outside a model run. Valid while `with_cancel_flag` runs.
pub(crate) fn cancel_arg() -> *mut std::ffi::c_void {
    RUN_CANCEL.with(|c| c.borrow().as_ref().map_or(std::ptr::null_mut(), |f| std::sync::Arc::as_ptr(f) as *mut std::ffi::c_void))
}

/// Error message used when a task stops because the user cancelled it.
pub const CANCELLED: &str = "Cancelled";

/// Ask a task to stop; it does so at its next check.
pub fn cancel(id: u64) {
    if let Some(t) = tasks().lock().get_mut(&id) {
        t.cancel_requested = true;
        t.detail = "Cancelling…".into();
    }
}

fn check_id(id: u64) -> anyhow::Result<()> {
    match tasks().lock().get(&id) {
        Some(t) if t.cancel_requested => anyhow::bail!(CANCELLED),
        _ => Ok(()),
    }
}

/// True for the error a cancelled task returns.
pub fn is_cancelled(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.to_string() == CANCELLED)
}

fn tasks() -> &'static Mutex<BTreeMap<u64, TaskInfo>> {
    static T: OnceLock<Mutex<BTreeMap<u64, TaskInfo>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

/// All running tasks, oldest first.
pub fn snapshot() -> Vec<TaskInfo> {
    tasks().lock().values().cloned().collect()
}

/// Handle for one running task; dropping it removes the task.
///
/// A task can be split into consecutive parts (`part(i, n)`), e.g. one per
/// downloaded dataset, and progress within a part maps into its share.
pub struct Task {
    id: u64,
    lo: f64,
    hi: f64,
}

impl Task {
    pub fn start(label: impl Into<String>) -> Task {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        tasks().lock().insert(id, TaskInfo { id, label: label.into(), fraction: Some(0.0), detail: String::new(), cancellable: false, cancel_requested: false });
        Task { id, lo: 0.0, hi: 1.0 }
    }

    /// Mark the task as one that honours [`Task::check`], so the UI shows Cancel.
    pub fn cancellable(self) -> Task {
        if let Some(t) = tasks().lock().get_mut(&self.id) {
            t.cancellable = true;
        }
        self
    }

    /// Err(CANCELLED) once the user has asked to cancel.
    pub fn check(&self) -> anyhow::Result<()> {
        check_id(self.id)
    }

    /// A cheap handle other threads can report through (absolute fractions).
    pub fn handle(&self) -> TaskHandle {
        TaskHandle { id: self.id }
    }

    /// Restrict later updates to the `i`-th of `n` equal parts.
    pub fn part(&mut self, i: usize, n: usize) {
        let n = n.max(1) as f64;
        self.lo = i as f64 / n;
        self.hi = (i as f64 + 1.0) / n;
        self.set(0.0, None::<&str>);
    }

    /// Progress within the current part (0..=1), with an optional detail line.
    pub fn set(&self, fraction: f64, detail: Option<impl Into<String>>) {
        let f = self.lo + (self.hi - self.lo) * fraction.clamp(0.0, 1.0);
        if let Some(t) = tasks().lock().get_mut(&self.id) {
            t.fraction = Some(f.max(t.fraction.unwrap_or(0.0)));
            if let Some(d) = detail {
                t.detail = d.into();
            }
        }
    }

    pub fn detail(&self, detail: impl Into<String>) {
        if let Some(t) = tasks().lock().get_mut(&self.id) {
            t.detail = detail.into();
        }
    }
}

/// Thread-safe reference to a running task (it may have ended; then updates are ignored).
#[derive(Clone, Copy, Debug)]
pub struct TaskHandle {
    id: u64,
}

impl TaskHandle {
    pub fn check(&self) -> anyhow::Result<()> {
        check_id(self.id)
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        tasks().lock().remove(&self.id);
    }
}
