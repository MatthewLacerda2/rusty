//! src/app/resources.rs — Resources: the engine singletons systems read and write.
//!
//! The Unity-style engine statics — one set per `GameWorld`: input, the nav graph,
//! the console, the active camera, the frame clock, the live rapier world, and the
//! script runtime — plus the play-mode bookkeeping the tick advances (play-state,
//! the frame counter, the edit-mode snapshot, the per-frame dt). Storage of record
//! (the `Scene`) is NOT here; it is threaded alongside `Resources` as the `&mut
//! Scene` world argument, so a system sees `(&mut Scene, &mut Resources)` — the
//! canonical two-argument form (issue #39). The borrow checker keeps the world
//! and the resources distinct at the system boundary.
//!
//! The engine-resource fields are PLAIN owned data — no `Rc`, no `RefCell` (#57).
//! Systems take `&mut res.nav` &c. directly. The mlua script runtime no longer
//! captures these with a `'static` lifetime: a script run opens a `lua.scope` over
//! the borrowed sim data, so the bindings reach the very same owned values the
//! systems hold, without any heap-shared interior mutability.
//!
//! Allowed deps: app::*, core, navigation, physics, render, scene, scripting, time.

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::SceneSnapshot;
use crate::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;

use super::Schedule;

/// The engine singletons (Unity's engine statics) plus the play-mode bookkeeping the
/// tick advances. Threaded into every system as the second argument, beside the
/// `&mut Scene` world. All fields are owned plain data; a script run borrows them
/// transiently through a `lua.scope`, and the editor / renderer borrow them too.
pub struct Resources {
    pub input: InputState,
    pub nav: NavigationGraph,
    pub console: ConsoleLogs,
    pub camera: Camera,
    pub time: Time,
    pub script_manager: ScriptManager,
    /// rapier3d simulation, rebuilt from the scene on Play and torn down on Stop.
    /// `None` in edit mode. Borrowed (not shared) into the script runtime so
    /// `Physics.Raycast`/`Shoot` cast against the very same world the engine
    /// hitscan does (#31).
    pub physics: Option<PhysicsWorld>,
    pub is_playing: bool,
    pub(super) was_playing: bool,
    pub(super) pathfinding_points: Vec<glam::Vec3>,
    /// Play-mode frame counter. Drives nav rebaking off a deterministic tick count
    /// instead of the wall clock, so a fixed-timestep replay is bit-for-bit stable.
    pub(super) play_frame: u64,
    /// Edit-mode scene captured on Play and restored on Stop, so play-mode mutations
    /// never leak back into the authoritative edit scene (Unity-style).
    pub(super) edit_snapshot: Option<SceneSnapshot>,
    /// The scaled per-frame delta (`Time::delta_time`) for the tick in flight, set by
    /// `GameWorld::tick` before the schedule runs. Systems read it instead of taking
    /// `dt` as a third argument, keeping the call shape `(&mut Scene, &mut Resources)`.
    pub(super) frame_dt: f32,
    /// The ordered per-stage system registry that drives the tick. Built once at
    /// construction from `app::build()`, where every module self-registers.
    pub(super) schedule: Schedule,
}

impl Resources {
    /// Build the resource set from owned engine state. The script runtime starts
    /// empty (its Lua state is created on Play) and borrows these fields per-run.
    pub fn new(input: InputState, nav: NavigationGraph, console: ConsoleLogs) -> Self {
        Self {
            input,
            nav,
            console,
            camera: Camera::new(glam::Vec3::new(0.0, 5.0, -10.0), 90.0, -20.0),
            time: Time::new(),
            script_manager: ScriptManager::new(),
            physics: None,
            is_playing: false,
            was_playing: false,
            pathfinding_points: Vec::new(),
            play_frame: 0,
            edit_snapshot: None,
            frame_dt: 0.0,
            schedule: super::build().into_schedule(),
        }
    }

    /// The scaled per-frame delta for the tick in flight (`Time::delta_time`).
    pub fn dt(&self) -> f32 {
        self.frame_dt
    }

    /// Number of play-mode frames simulated since the last `enter_play`.
    pub fn play_frame(&self) -> u64 {
        self.play_frame
    }

    /// The debug nav path computed by the play-mode nav system.
    pub fn pathfinding_points(&self) -> &[glam::Vec3] {
        &self.pathfinding_points
    }
}
