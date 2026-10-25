//! The cells every `api::` namespace borrows, in one owned bundle (#1384).
//!
//! [`ScriptManager`] keeps them as one [`ApiCells`] and lends them out per
//! evaluation as an [`ApiScopedCtx`]. A second VM that drives the same world from
//! the outside (the dev layer's scenario VM) cannot borrow the manager for that: it
//! holds its API scope for a whole run, while the world it observes ticks, and so
//! borrows the manager mutably, in between. It clones the bundle once
//! ([`ScriptManager::api_cells`]): the same `Rc`s, the same surface, one
//! registration, a second caller. A new cell is added here, once.

use std::cell::RefCell;
use std::rc::Rc;

use crate::api::ApiScopedCtx;
use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::frame_stats::FrameStats;
use crate::core::input::InputState;
use crate::core::random::Random;
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::video::VideoSettings;
use crate::core::storage::Storage;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::{Camera, Scene};
use crate::time::Time;
use crate::ui::{EventSystem, ScreenSize};

use super::{ConsoleLogs, ScriptManager, TimerScheduler};

/// The shared cells behind one [`ApiScopedCtx`], owned (see the module doc).
#[derive(Clone)]
pub struct ApiCells {
    pub(super) scene: Rc<RefCell<Scene>>,
    pub(super) input: Rc<RefCell<InputState>>,
    pub(super) nav: Rc<RefCell<NavigationGraph>>,
    pub(super) camera: Rc<RefCell<Camera>>,
    pub(super) time: Rc<RefCell<Time>>,
    /// Live rapier physics world shared with `GameWorld` — used by
    /// `Physics.Raycast`. `None` until Play builds the world into it: one cell for
    /// the manager's life, which `Resources` reads back (`physics_cell`).
    pub(super) physics: Rc<RefCell<Option<PhysicsWorld>>>,
    pub(super) console: Rc<RefCell<ConsoleLogs>>,
    /// The persistent key-value store, shared with the app so it can flush at
    /// boundaries. Injected by `Resources` via [`ScriptManager::set_storage`];
    /// defaults to an empty, pathless store (harness/tests never touch disk).
    pub(super) storage: Rc<RefCell<Storage>>,
    /// Global post-FX scalability tier (`Graphics.Get/SetQuality`). Shared with
    /// the platform layer, which reads it each frame and hands it to
    /// `renderer.set_quality`; defaults to `Medium` (harness/tests need no GPU).
    pub(super) quality: Rc<RefCell<QualityPreset>>,
    /// Runtime video settings (`Video.*`: resolution / vsync / fullscreen). Shared
    /// with the platform layer, which reads it each frame and reconfigures the
    /// surface + window; defaults to the windowed boot settings (harness needs no
    /// window).
    pub(super) video: Rc<RefCell<VideoSettings>>,
    /// The scene file the editor/session is currently editing — the write-back
    /// target for `Scene.Save()` (no-path). Shared with the platform layer (the
    /// editor keeps `EditorUi.current_scene_path` synced into it), so a scripted
    /// save and an editor save hit the same file. `None` until a scene is loaded
    /// or the path is set; an argument-less `Scene.Save()` then errors cleanly.
    pub(super) scene_path: Rc<RefCell<Option<String>>>,
    /// The audio engine singleton (#212), shared with `Resources` and the platform
    /// layer so the `Audio` namespace drives the same maestro the systems do.
    /// Defaults to a maestro with a no-op backend (harness/tests need no device).
    pub(super) audio: Rc<RefCell<AudioMaestro>>,
    /// The seeded gameplay RNG (#443) behind `Random` and the sandboxed
    /// `math.random`. Restarted from its fixed default seed by every
    /// [`ScriptManager::init_runtime`]: on every Play, and on every Stop that
    /// brings the edit-mode runtime back.
    pub(super) random: Rc<RefCell<Random>>,
    /// The screen the UI lays out on (#417), shared with `Resources` so `UI.GetRect`
    /// and the layout system agree on one size.
    pub(super) screen: Rc<RefCell<ScreenSize>>,
    /// Build settings + the quit request (#431), shared with `Resources` and the
    /// platform layer, which acts on `Application.Quit`. Defaults to unbound.
    pub(super) application: Rc<RefCell<Application>>,
    /// Frame stats (#433), filled by the dev layer and read by `Debug.Stats()`.
    pub(super) stats: Rc<RefCell<FrameStats>>,
    /// Pending script timers and coroutines (#444), behind the `Timer` namespace.
    /// Cleared with every fresh VM: its jobs hold keys into the old one.
    pub(super) timers: Rc<RefCell<TimerScheduler>>,
    /// The UI event system (#420): selection, hover and press state, shared with
    /// `Resources` (whose systems drive it) and the `UI` namespace. Reset every Play.
    pub(super) event_system: Rc<RefCell<EventSystem>>,
}

impl ApiCells {
    /// The given world cells, and a fresh default for every other one.
    pub(super) fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
        camera: Rc<RefCell<Camera>>,
        time: Rc<RefCell<Time>>,
    ) -> Self {
        Self {
            scene,
            input,
            nav,
            camera,
            time,
            physics: Rc::default(),
            console,
            storage: Rc::default(),
            quality: Rc::default(),
            video: Rc::default(),
            scene_path: Rc::default(),
            audio: Rc::default(),
            random: Rc::default(),
            screen: Rc::default(),
            application: Rc::new(RefCell::new(Application::new())),
            stats: Rc::default(),
            timers: Rc::default(),
            event_system: Rc::default(),
        }
    }

    /// The scoped context over these cells, as `api::register` takes it.
    pub fn ctx(&self) -> ApiScopedCtx<'_> {
        ApiScopedCtx {
            scene: &self.scene,
            input: &self.input,
            nav: &self.nav,
            camera: &self.camera,
            time: &self.time,
            physics: &self.physics,
            console: &self.console,
            storage: &self.storage,
            quality: &self.quality,
            video: &self.video,
            scene_path: &self.scene_path,
            audio: &self.audio,
            random: &self.random,
            screen: &self.screen,
            application: &self.application,
            stats: &self.stats,
            timers: &self.timers,
            event_system: &self.event_system,
        }
    }
}

impl ScriptManager {
    /// The cells this runtime's API borrows, for a VM that registers the same
    /// surface over the same world (see the module doc).
    pub fn api_cells(&self) -> ApiCells {
        self.cells.clone()
    }
}
