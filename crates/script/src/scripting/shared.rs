//! The cells every `api::` namespace borrows, owned in one bundle a second VM over the same
//! world clones. A new cell joins `ApiCells`, `ApiCells::attach`, `api::ApiCtx` and the list
//! in `tests::cells_attach`, which fails until it does.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use crate::api::bind;
use rusty_ai::query::QueryQueue;
use rusty_audio::AudioMaestro;
use rusty_core::application::Application;
use rusty_core::frame_stats::FrameStats;
use rusty_core::input::InputState;
use rusty_core::random::Random;
use rusty_core::settings::video::VideoSettings;
use rusty_core::settings::GraphicsSettings;
use rusty_core::storage::Storage;
use rusty_navigation::NavigationGraph;
use rusty_physics::PhysicsWorld;
use rusty_scene::{Camera, Scene};
use rusty_time::Time;
use rusty_ui::{EventSystem, ScreenSize};

use super::{ConsoleLogs, ScriptManager, TimerScheduler};

#[derive(Clone)]
pub struct ApiCells {
    pub(super) scene: Rc<RefCell<Scene>>,
    pub(super) input: Rc<RefCell<InputState>>,
    pub(super) nav: Rc<RefCell<NavigationGraph>>,
    pub(super) camera: Rc<RefCell<Camera>>,
    pub(super) time: Rc<RefCell<Time>>,
    pub(super) physics: Rc<RefCell<Option<PhysicsWorld>>>,
    pub(super) console: Rc<RefCell<ConsoleLogs>>,
    pub(super) storage: Rc<RefCell<Storage>>,
    pub(super) graphics: Rc<RefCell<GraphicsSettings>>,
    pub(super) video: Rc<RefCell<VideoSettings>>,
    pub(super) scene_path: Rc<RefCell<Option<String>>>,
    pub(super) audio: Rc<RefCell<AudioMaestro>>,
    pub(super) random: Rc<RefCell<Random>>,
    pub(super) screen: Rc<RefCell<ScreenSize>>,
    pub(super) application: Rc<RefCell<Application>>,
    pub(super) stats: Rc<RefCell<FrameStats>>,
    /// Cleared with every fresh VM: its jobs hold registry keys into the old one.
    pub(super) timers: Rc<RefCell<TimerScheduler>>,
    pub(super) event_system: Rc<RefCell<EventSystem>>,
    pub(super) queries: Rc<RefCell<QueryQueue>>,
}

impl ApiCells {
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
            graphics: Rc::default(),
            video: Rc::default(),
            scene_path: Rc::default(),
            audio: Rc::default(),
            random: Rc::default(),
            screen: Rc::default(),
            application: Rc::new(RefCell::new(Application::new())),
            stats: Rc::default(),
            timers: Rc::default(),
            event_system: Rc::default(),
            queries: Rc::default(),
        }
    }

    pub fn install(&self, lua: &Lua) -> Result<(), String> {
        self.attach(lua);
        crate::api::register(lua, &crate::api::ApiCtx::default())
    }

    pub(super) fn attach(&self, lua: &Lua) {
        // sorted: one cell per type, so the order is free
        bind(lua, &self.application);
        bind(lua, &self.audio);
        bind(lua, &self.camera);
        bind(lua, &self.console);
        bind(lua, &self.event_system);
        bind(lua, &self.graphics);
        bind(lua, &self.input);
        bind(lua, &self.nav);
        bind(lua, &self.physics);
        bind(lua, &self.queries);
        bind(lua, &self.random);
        bind(lua, &self.scene);
        bind(lua, &self.scene_path);
        bind(lua, &self.screen);
        bind(lua, &self.stats);
        bind(lua, &self.storage);
        bind(lua, &self.time);
        bind(lua, &self.timers);
        bind(lua, &self.video);
    }
}

impl ScriptManager {
    pub fn api_cells(&self) -> ApiCells {
        self.cells.clone()
    }
}

impl ScriptManager {
    pub fn serve_queries(&self) {
        let nav = self.cells.nav.borrow();
        let mut random = self.cells.random.borrow_mut();
        self.cells.queries.borrow_mut().serve(&nav, &mut random);
    }

    pub fn forget_queries(&self) {
        self.cells.queries.borrow_mut().clear();
    }
}
