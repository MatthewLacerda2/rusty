//! src/app/world.rs — World: the storage of record handed to every system.
//!
//! Issue #39 makes a system the canonical two-argument `fn(&mut World, &mut
//! Resources)`. `World` is the world half: the active `Scene` (the hecs-backed
//! storage of record, #40). `Resources` is the engine-singleton half. The two are
//! distinct types threaded side by side, so the borrow checker keeps world-storage
//! and engine-state from aliasing at the system boundary — no more one opaque
//! `GameWorld` blob the systems reach through.
//!
//! The `Scene` is still held behind an `Rc<RefCell<…>>`: the mlua script closures
//! capture that exact cell with a `'static` lifetime (they call `Scene.SetPosition`
//! &c. mid-tick), and the editor / renderer share it too. A system therefore reads
//! the scene through a short-lived `borrow()`/`borrow_mut()` — exactly as before —
//! rather than holding a long `&mut Scene` that would deadlock a re-entrant script
//! call. Lifting the script surface fully off `Rc<RefCell>` is the follow-up the
//! issue flags as its hardest part.
//!
//! Allowed deps: scene.

use std::cell::RefCell;
use std::rc::Rc;

use crate::scene::Scene;

/// The world of record threaded into every system as `&mut World`. A thin handle to
/// the active `Scene`; the `Rc<RefCell<…>>` is shared with the Lua runtime and the
/// editor, so systems borrow it transiently rather than holding it open across a
/// re-entrant script call.
pub struct World {
    pub scene: Rc<RefCell<Scene>>,
}

impl World {
    /// Wrap a shared scene handle as the world of record.
    pub fn new(scene: Rc<RefCell<Scene>>) -> Self {
        Self { scene }
    }
}
