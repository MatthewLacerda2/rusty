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
//! Allowed deps: app::*, core, navigation, physics, render, scene, scripting, time, ui.

use std::cell::RefCell;
use std::rc::Rc;

use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::input::InputState;
use crate::core::storage::Storage;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::Camera;
use crate::scene::{Scene, SceneSnapshot};
use crate::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;
use crate::ui::{EventSystem, ScreenSize, UiLayout};

use super::animation::graph::GraphCache;
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
    /// `Physics.Raycast` casts against the very same world the engine
    /// hitscan does (#31).
    pub physics: Rc<RefCell<Option<PhysicsWorld>>>,
    /// Persistent key-value store (issue #86). Shared with the script runtime; the
    /// platform layer loads it at startup ([`Storage::open`]) and flushes it at
    /// boundaries (Stop / quit). Empty + pathless in the harness, so headless runs
    /// never read a real save and stay reproducible.
    pub storage: Rc<RefCell<Storage>>,
    /// The screen the UI lays out on (#417) — a sim input. The windowed platform
    /// writes the game view's pixel size each frame; headless leaves it unset and
    /// the video resolution stands in. Shared with the `UI` namespace.
    pub screen: Rc<RefCell<ScreenSize>>,
    /// Every UI rect as of the end of the last tick (#417), in draw order —
    /// written by the `LateUpdate` layout system, read by the UI render pass
    /// (#418) and pointer dispatch (#420). Empty until the first Play tick.
    pub ui_layout: UiLayout,
    /// The UI event system (#420) — hover, press and focus state — shared with the
    /// script runtime's `UI` namespace. Reset on every Play.
    pub event_system: Rc<RefCell<EventSystem>>,
    /// Build settings + the quit request (#431), shared with the script runtime's
    /// `Application` namespace. Unbound by default; the platform layer loads it.
    pub application: Rc<RefCell<Application>>,
    pub is_playing: bool,
    pub(super) was_playing: bool,
    /// Play-mode frame counter. Drives nav rebaking off a deterministic tick count
    /// instead of the wall clock, so a fixed-timestep replay is bit-for-bit stable.
    pub(super) play_frame: u64,
    /// Edit-mode scene captured on Play and restored on Stop, so play-mode mutations
    /// never leak back into the authoritative edit scene (Unity-style).
    pub(super) edit_snapshot: Option<SceneSnapshot>,
    /// The scene file and identity editor Play started in, put back on Stop after a
    /// play-mode `Scene.Load` (#432). Taken with `edit_snapshot`.
    pub(super) play_origin: Option<super::scene_load::PlayOrigin>,
    /// A standalone run (the player, #431) has no edit mode to return to, so Play
    /// takes no edit snapshot. Set once by `GameWorld::boot_standalone`.
    pub(super) standalone: bool,
    /// The scaled per-frame delta (`Time::delta_time`) for the tick in flight, set by
    /// `GameWorld::tick` before the schedule runs. Systems read it instead of taking
    /// `dt` as a third argument, keeping the call shape `(&mut Scene, &mut Resources)`.
    pub(super) frame_dt: f32,
    /// The ordered per-stage system registry that drives the tick. Built once at
    /// construction from `app::build()`, where every module self-registers.
    pub(super) schedule: Schedule,
    /// Lazily-loaded `AnimationGraph` assets keyed by path (#316), shared by every
    /// entity referencing the same graph. Cleared on Play enter so per-session
    /// edits to the asset files are picked up.
    pub(super) animation_graphs: GraphCache,
    /// The schedule's observer (#433) — the dev layer's frame-stats timer. `None`
    /// unless installed via `GameWorld::set_probe`; it only watches the frame.
    pub(super) probe: Option<Box<dyn super::SystemProbe>>,
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
        // windowed app injects the real `KiraBackend` after construction. Shared
        // with the script runtime so the `Audio` namespace drives the same maestro.
        let audio = Rc::new(RefCell::new(AudioMaestro::default()));
        script_manager.set_audio_cell(Rc::clone(&audio));
        let screen = Rc::new(RefCell::new(ScreenSize::default()));
        script_manager.set_screen_cell(Rc::clone(&screen));
        let event_system = script_manager.event_system_cell();
        let application = Rc::new(RefCell::new(Application::new()));
        script_manager.set_application_cell(Rc::clone(&application));
        Self {
            input,
            nav,
            console,
            camera,
            time,
            audio,
            script_manager,
            storage,
            screen,
            ui_layout: UiLayout::default(),
            event_system,
            application,
            physics: Rc::new(RefCell::new(None)),
            is_playing: false,
            was_playing: false,
            play_frame: 0,
            edit_snapshot: None,
            play_origin: None,
            standalone: false,
            frame_dt: 0.0,
            schedule: super::build().into_schedule(),
            animation_graphs: GraphCache::default(),
            probe: None,
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

    /// The screen size in pixels the UI lays out on this tick: the platform's game
    /// view, else the video resolution (headless).
    pub fn screen_pixels(&self) -> glam::Vec2 {
        let video = self.script_manager.video_cell();
        let video = *video.borrow();
        self.screen.borrow().pixels(&video)
    }

    /// Number of play-mode frames simulated since the last `enter_play`.
    pub fn play_frame(&self) -> u64 {
        self.play_frame
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod resources_tests;
