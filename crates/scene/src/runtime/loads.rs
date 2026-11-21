use std::collections::BTreeMap;

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
            Self::Ready => 0.9,
            Self::Done => 1.0,
        }
    }

    pub fn is_done(self) -> bool {
        !matches!(self, Self::Loading | Self::Ready)
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
enum Pending {
    Sync(String),
    Async(AsyncOp),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Due {
    Sync(String),
    Async { op: u32, path: String },
}

#[derive(Debug, Default)]
pub struct SceneLoads {
    next_op: u32,
    pending: Option<Pending>,
    status: BTreeMap<u32, LoadStatus>,
    loaded: u8,
}

impl SceneLoads {
    pub fn request(&mut self, path: String) {
        self.replace();
        self.pending = Some(Pending::Sync(path));
    }

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

    pub fn ended(&mut self, status: LoadStatus) -> u32 {
        self.open(status)
    }

    fn open(&mut self, status: LoadStatus) -> u32 {
        self.next_op += 1;
        self.status.insert(self.next_op, status);
        self.next_op
    }

    fn replace(&mut self) {
        if let Some(Pending::Async(old)) = self.pending.take() {
            self.status.insert(old.op, LoadStatus::Replaced);
        }
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
        match &mut self.pending {
            Some(Pending::Async(pending)) if pending.op == op => {
                pending.allow = allow;
                true
            }
            _ => false,
        }
    }

    pub fn to_start(&mut self) -> Option<(u32, String)> {
        match &mut self.pending {
            Some(Pending::Async(op)) if !op.started => {
                op.started = true;
                Some((op.op, op.path.clone()))
            }
            _ => None,
        }
    }

    pub fn waiting(&self) -> Option<u32> {
        match &self.pending {
            Some(Pending::Async(op)) if self.status(op.op) == Some(LoadStatus::Loading) => {
                Some(op.op)
            }
            _ => None,
        }
    }

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

    pub fn finish(&mut self, op: u32, status: LoadStatus) {
        self.status.insert(op, status);
    }

    pub fn enter_play(&mut self) {
        self.next_op = 0;
        self.status.clear();
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn pending_op(&self) -> Option<u32> {
        match self.pending.as_ref()? {
            Pending::Async(op) => Some(op.op),
            Pending::Sync(_) => None,
        }
    }

    pub fn pending_path(&self) -> Option<&str> {
        match self.pending.as_ref()? {
            Pending::Sync(path) | Pending::Async(AsyncOp { path, .. }) => Some(path),
        }
    }
}
