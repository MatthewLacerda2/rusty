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
//! time one is set, so Lua that runs outside any verb pays nothing for it.
//!
//! **Off unless armed.** Extensions are process-wide and a running VM keeps the
//! surface it started with, so a run arms before its first VM and never disarms:
//! a plain run never wraps a thing, and its rows are what they always were.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::Instant;

mod wrap;

static ARMED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder::default());
    /// The system the schedule is running, set by the timing probe when armed.
    static SYSTEM: Cell<&'static str> = const { Cell::new("") };
}

/// The system whose verbs' time [`Frame::update_ms`] sums: `script.lua` is what
/// of its row no verb holds.
pub const UPDATE_SYSTEM: &str = "update_scripts";

/// Time every engine verb of every Lua VM started from now on, in this process.
/// Installs the dev surface first ([`install_api`](crate::lua_surface::install_api)),
/// so its verbs are wrapped too. Idempotent; there is no disarming.
pub fn arm() {
    static ARM: Once = Once::new();
    ARM.call_once(|| {
        crate::lua_surface::install_api();
        rusty_script::api::extend(wrap::register);
        ARMED.store(true, Ordering::Relaxed);
    });
}

/// Whether [`arm`] ran in this process.
pub fn armed() -> bool {
    ARMED.load(Ordering::Relaxed)
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
    /// For re-entered Lua: the calls open in it, counted by the hook.
    depth: u32,
}

#[derive(Default)]
struct Recorder {
    names: Vec<String>,
    /// Per verb id: exclusive ms and calls since the last take.
    totals: Vec<(f64, u32)>,
    update_ms: f64,
    stack: Vec<Open>,
}

impl Recorder {
    fn intern(&mut self, name: String) -> usize {
        self.names.push(name);
        self.totals.push((0.0, 0));
        self.names.len() - 1
    }

    /// Open verb `id`; its place on the stack, for [`Self::leave`].
    fn enter(&mut self, id: usize) -> usize {
        let start = Instant::now();
        let (verb, nested, depth) = (Some(id), 0.0, 0);
        self.stack.push(Open {
            verb,
            start,
            nested,
            depth,
        });
        self.stack.len() - 1
    }

    /// Close the verb opened at `at`, and whatever is still open above it (Lua
    /// an error unwound, whose returns the hook never saw).
    fn leave(&mut self, at: usize) {
        let now = Instant::now();
        while self.stack.len() > at {
            if let Some(open) = self.stack.pop() {
                self.close(open, now);
            }
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

    /// A call hook's event while a verb is open: a Lua function called with a
    /// verb on top is Lua re-entered, which then lasts until its calls return.
    fn event(&mut self, event: Event) {
        let Some(top) = self.stack.last_mut() else {
            return;
        };
        match (top.verb, event) {
            (Some(_), Event::Call { lua: true }) => {
                let (start, nested, depth) = (Instant::now(), 0.0, 1);
                let verb = None;
                self.stack.push(Open {
                    verb,
                    start,
                    nested,
                    depth,
                });
            }
            (None, Event::Call { .. }) => top.depth += 1,
            (None, Event::Return) => {
                top.depth = top.depth.saturating_sub(1);
                if top.depth == 0 {
                    let at = self.stack.len() - 1;
                    self.leave(at);
                }
            }
            _ => {}
        }
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
        Frame { verbs, update_ms }
    }
}

/// What the call hook saw, as the recorder needs it. A tail call replaces the
/// frame it leaves, so it opens nothing and is not passed.
#[derive(Clone, Copy, Debug)]
enum Event {
    Call { lua: bool },
    Return,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
