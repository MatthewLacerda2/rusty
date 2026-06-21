//! src/app/resources.rs — Resources: the engine singletons systems read and write.
//!
//! The Unity-style engine statics — one set per `GameWorld`: input, the nav graph,
//! the console, the active camera, the frame clock, the live rapier world, and the
//! script runtime — plus the play-mode bookkeeping the tick advances (play-state,
//! the frame counter, the edit-mode snapshot, the per-frame dt). Storage of record
//! (the `Scene`) is NOT here; it is threaded alongside `Resources` as the `&mut
//! Scene` world argument, so a system sees `(&mut Scene, &mut Resources)` — the
//! canonical two-argument form (issue #39). The borrow checker now keeps the world
//! and the resources distinct at the system boundary.
//!
//! The engine-resource fields are still `Rc<RefCell<…>>` handles: the mlua script
//! closures capture them with a `'static` lifetime, and the editor / renderer share
//! the very same cells. Converting that script-facing surface fully off `Rc<RefCell>`
//! is a follow-up (issue #39's hardest part). Threading `&mut Scene`/`&mut Resources`
//! through the system call path — done here — removes the borrow-panic risk from the
//! engine systems themselves: they no longer reach through one opaque blob.
//!
//! Allowed deps: app::*, core, navigation, physics, render, scene, scripting, time.

use std::cell::RefCell;
use std::rc::Rc;

use crate::audio::AudioMaestro;
use crate::core::input::InputState;
use crate::core::storage::Storage;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::{Scene, SceneSnapshot};
use crate::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;

use super::Schedule;

/// The engine singletons (Unity's engine statics) plus the play-mode bookkeeping the
/// tick advances. Threaded into every system as the second argument, beside the
/// `&mut Scene` world. The `Rc<RefCell<…>>` handles are shared with the Lua runtime
/// and the editor; the play-state scalars are owned outright.
pub struct Resources {
    pub input: Rc<RefCell<InputState>>,
    pub nav: Rc<RefCell<NavigationGraph>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub camera: Rc<RefCell<Camera>>,
    pub time: Rc<RefCell<Time>>,
    /// The audio engine singleton (#212). Owns the device/mixer (a no-op backend by
    /// default; the windowed app injects the real one) and the play-event log.
    /// Shared with the script runtime so the `Audio` namespace drives it.
    pub audio: Rc<RefCell<AudioMaestro>>,
    pub script_manager: ScriptManager,
    /// rapier3d simulation, rebuilt from the scene on Play and torn down on Stop.
    /// `None` in edit mode. Shared (via `Rc`) with the script runtime so
    /// `Physics.Raycast`/`Shoot` cast against the very same world the engine
    /// hitscan does (#31).
    pub physics: Rc<RefCell<Option<PhysicsWorld>>>,
    /// Persistent key-value store (issue #86). Shared with the script runtime; the
    /// platform layer loads it at startup ([`Storage::open`]) and flushes it at
    /// boundaries (Stop / quit). Empty + pathless in the harness, so headless runs
    /// never read a real save and stay reproducible.
    pub storage: Rc<RefCell<Storage>>,
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
    /// Build the resource set from the shared engine-state handles. The script
    /// runtime is wired to the same cells so live scripts and the engine agree.
    pub fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
        camera: Rc<RefCell<Camera>>,
        time: Rc<RefCell<Time>>,
    ) -> Self {
        let mut script_manager = ScriptManager::new(
            scene,
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
            Rc::clone(&camera),
            Rc::clone(&time),
        );
        // One store shared by the script runtime and the app. Pathless until the
        // platform layer binds it to a file at startup.
        let storage = Rc::new(RefCell::new(Storage::new()));
        script_manager.set_storage(Rc::clone(&storage));
        // The audio maestro starts with a no-op backend (the harness path); the
        // windowed app injects the real `RodioBackend` after construction. Shared
        // with the script runtime so the `Audio` namespace drives the same maestro.
        let audio = Rc::new(RefCell::new(AudioMaestro::default()));
        script_manager.set_audio_cell(Rc::clone(&audio));
        Self {
            input,
            nav,
            console,
            camera,
            time,
            audio,
            script_manager,
            storage,
            physics: Rc::new(RefCell::new(None)),
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

    /// Flush the persistent store at a boundary (Stop / quit), logging any failure
    /// to the console. A no-op when the store is pathless (harness/tests).
    pub fn flush_storage(&self) {
        if let Err(err) = self.storage.borrow().flush() {
            self.console
                .borrow_mut()
                .error(format!("Failed to flush storage: {}", err));
        }
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

#[cfg(test)]
#[path = "resources_tests.rs"]
mod resources_tests;
