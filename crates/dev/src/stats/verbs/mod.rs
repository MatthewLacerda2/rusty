//! crates/dev/src/stats/verbs/ — script time by engine verb (#2356), for
//! `make bench-scenes SCRIPT=1`.
//!
//! `cpu.update_scripts` holds the Lua VM and every engine call the scripts make.
//! [`arm`] adds an [`api::Extension`](rusty_script::api::Extension) that wraps each
//! function of every engine namespace (`Transform.SetPosition`,
//! `CharacterController.Move`, …) in a timing closure (`wrap`), so a run can say
//! how much of the row each verb is. Here, in the dev layer, because the sim reads
//! no clock: `rusty-script` never sees one.
//!
//! **Exclusive time.** A verb can call back into Lua (`Move` raises
//! `OnControllerColliderHit` inside the call), and that Lua can call more verbs.
//! A per-thread stack of open calls subtracts what ran nested: a verb's time is
//! its own engine work, the re-entered Lua is Lua's, and a nested verb is its own.
//! While a verb is open a call hook watches for Lua being re-entered, the only
//! time one is set, so Lua that runs outside any verb pays nothing for it. A
//! re-entered stretch ends when the function that opened it returns, found by its
//! depth on the Lua stack, so an error a handler catches itself (`pcall`), whose
//! unwound frames run no return hook, still closes it where it ends.
//!
//! **The wrappers' own cost** (setting the hook, the clock reads) is kept apart
//! as [`Frame::overhead_ms`], outside every verb's time and out of `script.lua`,
//! so the remainder is the VM running scripts and not the timers.
//!
//! **Off unless armed, per thread.** Extensions are process-wide and a running VM
//! keeps the surface it started with, so a run arms before its first VM. The
//! extension is installed once a process but wraps only the VMs a thread that
//! [`arm`]ed creates: a test that arms leaves every other thread's VMs bare, and a
//! plain run never wraps a thing, so its rows are what they always were.

use std::cell::{Cell, RefCell};
use std::sync::Once;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

mod wrap;

thread_local! {
    /// Whether this thread [`arm`]ed: the VMs it creates from then on are timed.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder::default());
    /// The system the schedule is running, set by the timing probe when armed.
    static SYSTEM: Cell<&'static str> = const { Cell::new("") };
}

#[cfg(test)]
thread_local! {
    /// A test's driven clock ([`drive`]): when it started and how far it moved.
    static DRIVEN: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
}

/// The dev layer's clock (verbs' and `TimingProbe`'s): the wall clock, or a test's.
pub(crate) fn now() -> Instant {
    #[cfg(test)]
    if let Some((start, moved)) = DRIVEN.with(Cell::get) {
        return start + moved;
    }
    Instant::now()
}

/// From now on this thread's recorder reads a clock that stands still until
/// [`advance`]d, so a test asserts exactly what each stretch is charged.
#[cfg(test)]
pub(crate) fn drive() {
    DRIVEN.with(|d| d.set(Some((Instant::now(), Duration::ZERO))));
}

/// Move the driven clock on by `ms`; nothing unless this thread [`drive`]s it.
#[cfg(test)]
pub(crate) fn advance(ms: u64) {
    DRIVEN.with(|d| {
        let moved = d
            .get()
            .map(|(start, at)| (start, at + Duration::from_millis(ms)));
        d.set(moved);
    });
}

/// The system whose verbs' time [`Frame::update_ms`] sums: `script.lua` is what
/// of its row neither a verb nor the wrappers hold.
pub const UPDATE_SYSTEM: &str = "update_scripts";

/// Time every engine verb of every Lua VM this thread starts from now on.
/// Installs the dev surface first ([`install_api`](crate::lua_surface::install_api)),
/// so its verbs are wrapped too. Idempotent; a thread never disarms, and the arm
/// ends with it.
pub fn arm() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        crate::lua_surface::install_api();
        rusty_script::api::extend(wrap::register);
    });
    ARMED.with(|a| a.set(true));
}

/// Whether this thread [`arm`]ed.
pub fn armed() -> bool {
    ARMED.with(Cell::get)
}

/// The schedule is about to run the system `name` (the timing probe calls it).
pub(crate) fn enter_system(name: &'static str) {
    if armed() {
        SYSTEM.with(|s| s.set(name));
    }
}

/// One verb's share of the frames since the last [`take`].
#[derive(Clone, Debug, PartialEq)]
pub struct Verb {
    /// `Namespace.Function`.
    pub name: String,
    /// Exclusive time: the verb's own, without the Lua or verbs it ran nested.
    pub ms: f64,
    pub calls: u32,
}

/// What the verbs did since the last [`take`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// Every verb called, in the order first wrapped.
    pub verbs: Vec<Verb>,
    /// The verbs' exclusive time inside [`UPDATE_SYSTEM`].
    pub update_ms: f64,
    /// The wrappers' own time inside [`UPDATE_SYSTEM`]: neither a verb's nor Lua's.
    pub overhead_ms: f64,
}

/// The verbs' time and calls since the last call, and start afresh. Empty when
/// not [`armed`].
pub fn take() -> Frame {
    RECORDER.with_borrow_mut(Recorder::take)
}

/// One open call: a verb, or a stretch of Lua it re-entered.
struct Open {
    /// The verb's id, `None` for re-entered Lua.
    verb: Option<usize>,
    start: Instant,
    /// The time of what ran nested in it, in ms.
    nested: f64,
    /// For re-entered Lua: the Lua stack's depth when the function that opened
    /// it was called; the stretch ends when that function returns or is gone.
    level: usize,
}

#[derive(Default)]
struct Recorder {
    names: Vec<String>,
    /// Per verb id: exclusive ms and calls since the last take.
    totals: Vec<(f64, u32)>,
    update_ms: f64,
    overhead_ms: f64,
    stack: Vec<Open>,
}

/// What is on top of the recorder's stack, as the call hook needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Top {
    Nothing,
    Verb,
    /// Re-entered Lua, opened at this stack depth.
    Lua(usize),
}

impl Recorder {
    /// The id of verb `name`: one id a name, however many VMs wrap it, so a
    /// window spanning two VMs sums both.
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

    /// Open verb `id`; its place on the stack, for [`Self::leave`], and when.
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

    /// Close the verb opened at `at`, and whatever is still open above it (Lua
    /// an error unwound, whose returns the hook never saw); when it closed.
    fn leave(&mut self, at: usize) -> Instant {
        let now = now();
        while self.stack.len() > at {
            if let Some(open) = self.stack.pop() {
                self.close(open, now);
            }
        }
        now
    }

    /// A wrapper spent `ms` of its own around its verb: kept apart, and out of
    /// the time of the verb it ran nested in.
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

    /// A Lua function a verb called back, at stack depth `level`: re-entered
    /// Lua, which lasts until that function returns.
    fn open_lua(&mut self, level: usize) {
        let (start, nested, verb) = (now(), 0.0, None);
        self.stack.push(Open {
            verb,
            start,
            nested,
            level,
        });
    }

    /// A hook event at stack depth `depth`, with re-entered Lua on top: the
    /// stretch ends at its opener's return, or at a call no deeper than it (the
    /// opener is gone: an error unwound it and the verb went on), and the hook
    /// then opens the next one if that call re-enters. Whether it ended, with a
    /// verb on top now.
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

/// What the call hook saw, as the recorder needs it. A tail call replaces the
/// frame it leaves at the same depth, so it opens nothing and is not passed.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    Call,
    Return,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
