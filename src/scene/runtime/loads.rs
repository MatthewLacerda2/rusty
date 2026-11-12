//! src/scene/runtime/loads.rs — the scene's load requests (#432, #2123): Unity's
//! `SceneManager.LoadScene` and `LoadSceneAsync` with its `AsyncOperation`.
//!
//! One load is pending at a time. `Scene.Load` queues a path, swapped in at the
//! tick's tail. `Scene.LoadAsync` opens an *operation* instead, an integer id a
//! script polls: it is prepared from its files off the tick (the host's
//! `app::SceneLoader`) and swapped in at the tail of a later tick, once it is ready
//! and its activation allowed. A later request of either kind replaces a pending
//! one, as the last `Scene.Load` in a tick wins.
//!
//! This is the sim's side only: plain state, changed by the API, by the host at a
//! tick head ([`SceneLoads::publish`], what the worker reported, an input like
//! `InputFrame`) and by the load phase at the tail. Nothing here waits or polls.

use std::collections::BTreeMap;

/// Where a load operation stands, as a script reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadStatus {
    /// Being prepared from its files.
    Loading,
    /// Prepared, waiting for its activation (`allowSceneActivation`) or the tick's tail.
    Ready,
    /// Swapped in: the active scene.
    Done,
    /// Its file could not be read or parsed; the scene that ran keeps running.
    Failed,
    /// Another load replaced it before it was swapped in.
    Replaced,
}

impl LoadStatus {
    /// The status as the API names it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Replaced => "replaced",
        }
    }

    /// Unity's `AsyncOperation.progress`: 0 while loading, 0.9 once ready (it holds
    /// there while activation waits), 1 once swapped in; 0 for a load that ended
    /// without one.
    pub fn progress(self) -> f64 {
        match self {
            Self::Loading | Self::Failed | Self::Replaced => 0.0,
            Self::Ready => 0.9,
            Self::Done => 1.0,
        }
    }

    /// Unity's `isDone`: the operation will not move again.
    pub fn is_done(self) -> bool {
        !matches!(self, Self::Loading | Self::Ready)
    }
}

/// An operation not yet swapped in.
#[derive(Debug)]
struct AsyncOp {
    op: u32,
    path: String,
    /// Whether the host was handed it ([`SceneLoads::to_start`]).
    started: bool,
    allow: bool,
}

#[derive(Debug)]
enum Pending {
    Sync(String),
    Async(AsyncOp),
}

/// A load the tick's tail swaps in now ([`SceneLoads::take_due`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Due {
    /// A `Scene.Load`: prepared and activated at once.
    Sync(String),
    /// A ready `Scene.LoadAsync` operation, its prepared scene held by the host.
    Async { op: u32, path: String },
}

/// The scene's load requests and the operations' statuses, for the Play session.
#[derive(Debug, Default)]
pub struct SceneLoads {
    next_op: u32,
    pending: Option<Pending>,
    status: BTreeMap<u32, LoadStatus>,
    /// How far the waiting operation is, in percent, as the host last published it.
    loaded: u8,
}

impl SceneLoads {
    /// Queue a `Scene.Load` of `path`, replacing any pending load.
    pub fn request(&mut self, path: String) {
        self.replace();
        self.pending = Some(Pending::Sync(path));
    }

    /// Open a `Scene.LoadAsync` operation for `path`, replacing any pending load.
    pub fn request_async(&mut self, path: String) -> u32 {
        self.replace();
        self.loaded = 0;
        let op = self.open(LoadStatus::Loading);
        let (started, allow) = (false, true);
        self.pending = Some(Pending::Async(AsyncOp {
            op,
            path,
            started,
            allow,
        }));
        op
    }

    /// An operation that ended as it opened (an edit-mode `LoadAsync`, loaded at once).
    pub fn ended(&mut self, status: LoadStatus) -> u32 {
        self.open(status)
    }

    fn open(&mut self, status: LoadStatus) -> u32 {
        self.next_op += 1;
        self.status.insert(self.next_op, status);
        self.next_op
    }

    /// A pending operation is replaced by a newer request.
    fn replace(&mut self) {
        if let Some(Pending::Async(old)) = self.pending.take() {
            self.status.insert(old.op, LoadStatus::Replaced);
        }
    }

    /// Where operation `op` stands; `None` for one never opened this session.
    pub fn status(&self, op: u32) -> Option<LoadStatus> {
        self.status.get(&op).copied()
    }

    /// Unity's `AsyncOperation.progress` of `op`: a waiting operation's moves from 0
    /// toward 0.9 as the host publishes it ([`Self::set_loaded`]), then as
    /// [`LoadStatus::progress`]; `None` for one never opened this session.
    pub fn progress(&self, op: u32) -> Option<f64> {
        match self.waiting() == Some(op) {
            true => Some(0.9 * f64::from(self.loaded) / 100.0),
            false => self.status(op).map(LoadStatus::progress),
        }
    }

    /// The waiting operation's progress, in percent, as last published.
    pub fn loaded(&self) -> u8 {
        self.loaded
    }

    /// The host's report at a tick head: the waiting operation `op` is `percent`
    /// (0 to 100) of the way to ready. Ignored for any other operation.
    pub fn set_loaded(&mut self, op: u32, percent: u8) {
        if self.waiting() == Some(op) {
            self.loaded = percent.min(100);
        }
    }

    /// Unity's `allowSceneActivation` for the pending operation `op`: `false` when
    /// `op` is not pending.
    pub fn allow_activation(&mut self, op: u32, allow: bool) -> bool {
        match &mut self.pending {
            Some(Pending::Async(pending)) if pending.op == op => {
                pending.allow = allow;
                true
            }
            _ => false,
        }
    }

    /// The pending operation the host has not been handed yet, marked handed.
    pub fn to_start(&mut self) -> Option<(u32, String)> {
        match &mut self.pending {
            Some(Pending::Async(op)) if !op.started => {
                op.started = true;
                Some((op.op, op.path.clone()))
            }
            _ => None,
        }
    }

    /// The pending operation still being prepared.
    pub fn waiting(&self) -> Option<u32> {
        match &self.pending {
            Some(Pending::Async(op)) if self.status(op.op) == Some(LoadStatus::Loading) => {
                Some(op.op)
            }
            _ => None,
        }
    }

    /// The host's report at a tick head: operation `op` finished preparing, `ok` or
    /// failed. A failed one is no longer pending.
    pub fn publish(&mut self, op: u32, ok: bool) {
        if self.waiting() != Some(op) {
            return;
        }
        if ok {
            self.status.insert(op, LoadStatus::Ready);
        } else {
            self.status.insert(op, LoadStatus::Failed);
            self.pending = None;
        }
    }

    /// The load the tick's tail swaps in now: a queued `Scene.Load`, or the
    /// operation that is ready with its activation allowed.
    pub fn take_due(&mut self) -> Option<Due> {
        let due = match self.pending.as_ref()? {
            Pending::Sync(path) => Due::Sync(path.clone()),
            Pending::Async(op) if op.allow && self.status(op.op) == Some(LoadStatus::Ready) => {
                Due::Async {
                    op: op.op,
                    path: op.path.clone(),
                }
            }
            Pending::Async(_) => return None,
        };
        self.pending = None;
        Some(due)
    }

    /// Operation `op` was swapped in (`Done`), or could not be (`Failed`).
    pub fn finish(&mut self, op: u32, status: LoadStatus) {
        self.status.insert(op, status);
    }

    /// Play is entered: operations are numbered from 1 again, so a demo's ids are its
    /// replay's. A queued `Scene.Load` stays queued.
    pub fn enter_play(&mut self) {
        self.next_op = 0;
        self.status.clear();
    }

    /// Whether a load is pending.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The pending `Scene.LoadAsync` operation, ready or not.
    pub fn pending_op(&self) -> Option<u32> {
        match self.pending.as_ref()? {
            Pending::Async(op) => Some(op.op),
            Pending::Sync(_) => None,
        }
    }

    /// The scene file the pending load is of.
    pub fn pending_path(&self) -> Option<&str> {
        match self.pending.as_ref()? {
            Pending::Sync(path) | Pending::Async(AsyncOp { path, .. }) => Some(path),
        }
    }
}
