use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use mlua::Lua;

use crate::scripting::{ConsoleLogs, TimerScheduler};
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

pub struct ApiCell<T: 'static>(PhantomData<fn() -> T>);

impl<T: 'static> ApiCell<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }

    /// Clones the `Rc` and drops the app-data borrow before returning: `set_app_data`
    /// panics while one is held, and a callback may run it.
    pub fn get(self, lua: &Lua) -> mlua::Result<Rc<RefCell<T>>> {
        match lua.app_data_ref::<Rc<RefCell<T>>>() {
            Some(cell) => Ok(Rc::clone(&cell)),
            None => Err(mlua::Error::RuntimeError(format!(
                "no {} is attached to this Lua VM",
                std::any::type_name::<T>()
            ))),
        }
    }

    pub fn with<R>(
        self,
        lua: &Lua,
        f: impl FnOnce(&RefCell<T>) -> mlua::Result<R>,
    ) -> mlua::Result<R> {
        f(&*self.get(lua)?)
    }
}

impl<T: 'static> Clone for ApiCell<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for ApiCell<T> {}

impl<T: 'static> Default for ApiCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn bind<T: 'static>(lua: &Lua, cell: &Rc<RefCell<T>>) -> ApiCell<T> {
    lua.set_app_data(Rc::clone(cell));
    ApiCell::new()
}

/// Every field has its own type: app data is keyed by type, so a second cell of one type
/// replaces the first.
#[derive(Clone, Copy, Default)]
pub struct ApiCtx {
    pub scene: ApiCell<Scene>,
    pub input: ApiCell<InputState>,
    pub nav: ApiCell<NavigationGraph>,
    pub camera: ApiCell<Camera>,
    pub time: ApiCell<Time>,
    pub physics: ApiCell<Option<PhysicsWorld>>,
    pub console: ApiCell<ConsoleLogs>,
    pub storage: ApiCell<Storage>,
    pub graphics: ApiCell<GraphicsSettings>,
    pub video: ApiCell<VideoSettings>,
    pub scene_path: ApiCell<Option<String>>,
    pub audio: ApiCell<AudioMaestro>,
    pub random: ApiCell<Random>,
    pub screen: ApiCell<ScreenSize>,
    pub application: ApiCell<Application>,
    pub stats: ApiCell<FrameStats>,
    pub timers: ApiCell<TimerScheduler>,
    pub event_system: ApiCell<EventSystem>,
    pub queries: ApiCell<QueryQueue>,
}

#[cfg(test)]
pub(crate) fn attempt<R>(body: impl FnOnce() -> mlua::Result<R>) -> mlua::Result<R> {
    body()
}
