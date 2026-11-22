use std::cell::{Cell, RefCell};
use std::sync::Once;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

mod wrap;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder::default());
    static SYSTEM: Cell<&'static str> = const { Cell::new("") };
}

#[cfg(test)]
thread_local! {
    static DRIVEN: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
}

pub(crate) fn now() -> Instant {
    #[cfg(test)]
    if let Some((start, moved)) = DRIVEN.with(Cell::get) {
        return start + moved;
    }
    Instant::now()
}

#[cfg(test)]
pub(crate) fn drive() {
    DRIVEN.with(|d| d.set(Some((Instant::now(), Duration::ZERO))));
}

#[cfg(test)]
pub(crate) fn advance(ms: u64) {
    DRIVEN.with(|d| {
        let moved = d
            .get()
            .map(|(start, at)| (start, at + Duration::from_millis(ms)));
        d.set(moved);
    });
}

/// The fn name of `app::play`'s script system, which names its row: rename both
/// together, or `update_ms` and `overhead_ms` read 0.
pub const UPDATE_SYSTEM: &str = "update_scripts";

/// Arm before the thread's first VM: a running VM keeps the surface it started with.
pub fn arm() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        crate::lua_surface::install_api();
        rusty_script::api::extend(wrap::register);
    });
    ARMED.with(|a| a.set(true));
}

pub fn armed() -> bool {
    ARMED.with(Cell::get)
}

pub(crate) fn enter_system(name: &'static str) {
    if armed() {
        SYSTEM.with(|s| s.set(name));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Verb {
    pub name: String,
    pub ms: f64,
    pub calls: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub verbs: Vec<Verb>,
    pub update_ms: f64,
    pub overhead_ms: f64,
}

pub fn take() -> Frame {
    RECORDER.with_borrow_mut(Recorder::take)
}

struct Open {
    verb: Option<usize>,
    start: Instant,
    nested: f64,
    level: usize,
}

#[derive(Default)]
struct Recorder {
    names: Vec<String>,
    totals: Vec<(f64, u32)>,
    update_ms: f64,
    overhead_ms: f64,
    stack: Vec<Open>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Top {
    Nothing,
    Verb,
    Lua(usize),
}

impl Recorder {
    fn intern(&mut self, name: String) -> usize {
        if let Some(id) = self.names.iter().position(|n| *n == name) {
            return id;
        }
        self.names.push(name);
        self.totals.push((0.0, 0));
        self.names.len() - 1
    }

    fn top(&self) -> Top {
        match self.stack.last() {
            None => Top::Nothing,
            Some(Open { verb: Some(_), .. }) => Top::Verb,
            Some(open) => Top::Lua(open.level),
        }
    }

    fn enter(&mut self, id: usize) -> (usize, Instant) {
        let start = now();
        let (verb, nested, level) = (Some(id), 0.0, 0);
        self.stack.push(Open {
            verb,
            start,
            nested,
            level,
        });
        (self.stack.len() - 1, start)
    }

    fn leave(&mut self, at: usize) -> Instant {
        let now = now();
        while self.stack.len() > at {
            if let Some(open) = self.stack.pop() {
                self.close(open, now);
            }
        }
        now
    }

    fn overhead(&mut self, ms: f64) {
        if SYSTEM.with(Cell::get) == UPDATE_SYSTEM {
            self.overhead_ms += ms;
        }
        if let Some(parent) = self.stack.last_mut() {
            parent.nested += ms;
        }
    }

    fn close(&mut self, open: Open, now: Instant) {
        let total = now.duration_since(open.start).as_secs_f64() * 1000.0;
        if let Some(id) = open.verb {
            let own = (total - open.nested).max(0.0);
            let (ms, calls) = &mut self.totals[id];
            *ms += own;
            *calls += 1;
            if SYSTEM.with(Cell::get) == UPDATE_SYSTEM {
                self.update_ms += own;
            }
        }
        if let Some(parent) = self.stack.last_mut() {
            parent.nested += total;
        }
    }

    fn open_lua(&mut self, level: usize) {
        let (start, nested, verb) = (now(), 0.0, None);
        self.stack.push(Open {
            verb,
            start,
            nested,
            level,
        });
    }

    /// A stretch ends at its opener's return, or at a call no deeper: an error unwound
    /// the opener, and an unwound frame runs no return hook.
    fn lua_event(&mut self, depth: usize) -> bool {
        let Top::Lua(level) = self.top() else {
            return false;
        };
        if depth > level {
            return false;
        }
        let at = self.stack.len() - 1;
        self.leave(at);
        self.top() == Top::Verb
    }

    fn take(&mut self) -> Frame {
        let verbs = self
            .names
            .iter()
            .zip(&mut self.totals)
            .filter(|(_, (_, calls))| *calls > 0)
            .map(|(name, total)| {
                let (ms, calls) = std::mem::take(total);
                let name = name.clone();
                Verb { name, ms, calls }
            })
            .collect();
        let update_ms = std::mem::take(&mut self.update_ms);
        let overhead_ms = std::mem::take(&mut self.overhead_ms);
        Frame {
            verbs,
            update_ms,
            overhead_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    Call,
    Return,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
