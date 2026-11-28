//! The sim's side of scene loading: plain state changed by the API, by the host at a tick head and
//! by the load phase at the tail. Nothing here waits or polls.

use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadStatus {
    Loading,
    Ready,
    Done,
    Failed,
    Replaced,
}

impl LoadStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Replaced => "replaced",
        }
    }

    pub fn progress(self) -> f64 {
        match self {
            Self::Loading | Self::Failed | Self::Replaced => 0.0,
            // Unity's `AsyncOperation.progress`: ready, held while activation waits.
            Self::Ready => 0.9,
            Self::Done => 1.0,
        }
    }

    pub fn is_done(self) -> bool {
        !matches!(self, Self::Loading | Self::Ready)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadMode {
    #[default]
    Single,
    Additive,
}

impl LoadMode {
    pub const NAMES: [&'static str; 2] = ["Single", "Additive"];

    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "single" => Some(Self::Single),
            "additive" => Some(Self::Additive),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct AsyncOp {
    op: u32,
    path: String,
    started: bool,
    allow: bool,
}

#[derive(Debug)]
enum Kind {
    Sync(String),
    Async(AsyncOp),
}

#[derive(Debug)]
struct Pending {
    kind: Kind,
    mode: LoadMode,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Due {
    Sync(String, LoadMode),
    Async {
        op: u32,
        path: String,
        mode: LoadMode,
    },
}

/// In request order: only the first is prepared, one activates a tick tail, and a `Single`
/// request replaces them all.
#[derive(Debug, Default)]
pub struct SceneLoads {
    next_op: u32,
    pending: VecDeque<Pending>,
    status: BTreeMap<u32, LoadStatus>,
    /// The waiting operation's progress, 0 to 100 percent, as the host last published it.
    loaded: u8,
}

impl SceneLoads {
    pub fn request(&mut self, path: String) {
        self.request_in(path, LoadMode::Single);
    }

    pub fn request_in(&mut self, path: String, mode: LoadMode) {
        self.queue(Kind::Sync(path), mode);
    }

    pub fn request_async(&mut self, path: String) -> u32 {
        self.request_async_in(path, LoadMode::Single)
    }

    pub fn request_async_in(&mut self, path: String, mode: LoadMode) -> u32 {
        let op = self.open(LoadStatus::Loading);
        let (started, allow) = (false, true);
        let kind = Kind::Async(AsyncOp {
            op,
            path,
            started,
            allow,
        });
        self.queue(kind, mode);
        op
    }

    fn queue(&mut self, kind: Kind, mode: LoadMode) {
        if mode == LoadMode::Single {
            self.replace();
        }
        if self.pending.is_empty() {
            self.loaded = 0;
        }
        self.pending.push_back(Pending { kind, mode });
    }

    pub fn ended(&mut self, status: LoadStatus) -> u32 {
        self.open(status)
    }

    fn open(&mut self, status: LoadStatus) -> u32 {
        self.next_op += 1;
        self.status.insert(self.next_op, status);
        self.next_op
    }

    fn replace(&mut self) {
        for old in std::mem::take(&mut self.pending) {
            if let Kind::Async(old) = old.kind {
                self.status.insert(old.op, LoadStatus::Replaced);
            }
        }
    }

    fn front(&self) -> Option<&Kind> {
        self.pending.front().map(|p| &p.kind)
    }

    fn pop(&mut self) {
        self.pending.pop_front();
        self.loaded = 0;
    }

    pub fn status(&self, op: u32) -> Option<LoadStatus> {
        self.status.get(&op).copied()
    }

    pub fn progress(&self, op: u32) -> Option<f64> {
        match self.waiting() == Some(op) {
            true => Some(0.9 * f64::from(self.loaded) / 100.0),
            false => self.status(op).map(LoadStatus::progress),
        }
    }

    pub fn loaded(&self) -> u8 {
        self.loaded
    }

    pub fn set_loaded(&mut self, op: u32, percent: u8) {
        if self.waiting() == Some(op) {
            self.loaded = percent.min(100);
        }
    }

    pub fn allow_activation(&mut self, op: u32, allow: bool) -> bool {
        let ops = self.pending.iter_mut().filter_map(|p| match &mut p.kind {
            Kind::Async(pending) => Some(pending),
            Kind::Sync(_) => None,
        });
        match ops.into_iter().find(|pending| pending.op == op) {
            Some(pending) => {
                pending.allow = allow;
                true
            }
            None => false,
        }
    }

    pub fn to_start(&mut self) -> Option<(u32, String)> {
        match self.pending.front_mut().map(|p| &mut p.kind) {
            Some(Kind::Async(op)) if !op.started => {
                op.started = true;
                Some((op.op, op.path.clone()))
            }
            _ => None,
        }
    }

    pub fn waiting(&self) -> Option<u32> {
        match self.front() {
            Some(Kind::Async(op)) if self.status(op.op) == Some(LoadStatus::Loading) => Some(op.op),
            _ => None,
        }
    }

    /// `ok` false: preparing failed, and the operation is no longer pending.
    pub fn publish(&mut self, op: u32, ok: bool) {
        if self.waiting() != Some(op) {
            return;
        }
        if ok {
            self.status.insert(op, LoadStatus::Ready);
        } else {
            self.status.insert(op, LoadStatus::Failed);
            self.pop();
        }
    }

    pub fn take_due(&mut self) -> Option<Due> {
        let first = self.pending.front()?;
        let mode = first.mode;
        let due = match &first.kind {
            Kind::Sync(path) => Due::Sync(path.clone(), mode),
            Kind::Async(op) if op.allow && self.status(op.op) == Some(LoadStatus::Ready) => {
                Due::Async {
                    op: op.op,
                    path: op.path.clone(),
                    mode,
                }
            }
            Kind::Async(_) => return None,
        };
        self.pop();
        Some(due)
    }

    pub fn finish(&mut self, op: u32, status: LoadStatus) {
        self.status.insert(op, status);
    }

    /// Operations count from 1 again, so a demo's ids are its replay's. A queued load stays.
    pub fn enter_play(&mut self) {
        self.next_op = 0;
        self.status.clear();
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn pending_op(&self) -> Option<u32> {
        match self.front()? {
            Kind::Async(op) => Some(op.op),
            Kind::Sync(_) => None,
        }
    }

    pub fn pending_path(&self) -> Option<&str> {
        match self.front()? {
            Kind::Sync(path) | Kind::Async(AsyncOp { path, .. }) => Some(path),
        }
    }
}
