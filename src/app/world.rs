//! src/app/world.rs — World: the storage of record handed to every system.
//!
//! Issue #39 makes a system the canonical two-argument `fn(&mut World, &mut
//! Resources)`. `World` is the world half: the active `Scene` (the hecs-backed
//! storage of record, #40). `Resources` is the engine-singleton half. The two are
//! distinct types threaded side by side, so the borrow checker keeps world-storage
//! and engine-state from aliasing at the system boundary — no more one opaque
//! `GameWorld` blob the systems reach through.
//!
//! The `Scene` is owned outright — plain data, no `Rc<RefCell>` (#57). A system
//! takes `&mut world.scene` directly. The Lua runtime no longer captures the scene
//! with a `'static` lifetime; instead it runs inside a `lua.scope` over a borrowed
//! `&mut Scene`, so re-entrant `Scene.SetPosition` &c. calls reach the very same
//! scene the system is holding without any heap-shared interior mutability.
//!
//! Allowed deps: scene.

use crate::scene::Scene;

/// The world of record threaded into every system as `&mut World`. Owns the active
/// `Scene` outright; systems take `&mut world.scene` directly, and the script
/// runtime borrows it through a scoped callback for the duration of a run.
pub struct World {
    pub scene: Scene,
}

impl World {
    /// Wrap an owned scene as the world of record.
    pub fn new(scene: Scene) -> Self {
        Self { scene }
    }
}
