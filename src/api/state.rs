//! How the surface's functions reach engine state (#1709).
//!
//! The whole surface is registered **once per VM**, as plain `'static` Lua
//! functions, the way Unity binds its internal calls once per domain. A function
//! reaches the engine through state attached to its VM: each shared cell is stored
//! as Lua app data, keyed by its type, and an [`ApiCell`] handle fetches it on every
//! call. The handle is a zero-sized marker, so no function captures an `Rc` (#175's
//! intent): the VM's app data owns the one strong reference, and a fresh VM or a
//! re-attached cell is seen by every function from the next call on.
//!
//! [`ApiCell::get`] hands back its own `Rc` clone and releases the app-data borrow
//! before returning, so a function that calls back into Lua (a timer, a tween, a
//! hook firing another hook) holds nothing the inner call could trip on.
//!
//! One cell per type: two cells of one type would alias. Every cell the surface
//! reads has a distinct type ([`ApiCtx`] lists them), and
//! `scripting::tests::cells_attach` fails on a cell left unattached, on two of
//! one type, and on a registrar that reads a cell while registering.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use mlua::Lua;

use crate::ai::query::QueryQueue;
use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::frame_stats::FrameStats;
use crate::core::input::InputState;
use crate::core::random::Random;
use crate::core::settings::video::VideoSettings;
use crate::core::settings::GraphicsSettings;
use crate::core::storage::Storage;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::{Camera, Scene};
use crate::scripting::{ConsoleLogs, TimerScheduler};
use crate::time::Time;
use crate::ui::{EventSystem, ScreenSize};

/// A handle to the shared `RefCell<T>` attached to the calling VM (see the module
/// doc). Zero-sized and `Copy`, so a registered function captures it for free.
pub struct ApiCell<T: 'static>(PhantomData<fn() -> T>);

impl<T: 'static> ApiCell<T> {
    /// The handle to the VM's `T` cell.
    pub const fn new() -> Self {
        Self(PhantomData)
    }

    /// The `T` cell attached to `lua`; an error when none is (a VM that registered
    /// the surface without attaching its state).
    pub fn get(self, lua: &Lua) -> mlua::Result<Rc<RefCell<T>>> {
        match lua.app_data_ref::<Rc<RefCell<T>>>() {
            Some(cell) => Ok(Rc::clone(&cell)),
            None => Err(mlua::Error::RuntimeError(format!(
                "no {} is attached to this Lua VM",
                std::any::type_name::<T>()
            ))),
        }
    }

    /// Run `f` on the `T` cell attached to `lua` ([`Self::get`]). The cell comes in
    /// as `f`'s parameter, so a result that borrows it (`Ok(cell.borrow().x)`)
    /// drops that borrow before the cell is released.
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

/// Attach `cell` to `lua` as the one `T` its functions reach, replacing any earlier
/// one, and return its handle. Called from Rust, between Lua calls.
pub fn bind<T: 'static>(lua: &Lua, cell: &Rc<RefCell<T>>) -> ApiCell<T> {
    lua.set_app_data(Rc::clone(cell));
    ApiCell::new()
}

/// The handle to every cell the surface reads, as [`super::register`] hands them to
/// the namespace registrars. Each registrar names in its signature the cells its
/// functions read; the handles carry no data.
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
    /// Post-FX tier + player options, shared with the platform layer so a
    /// `Graphics.SetQuality`/`SetOption` write reaches the renderer (#1438).
    pub graphics: ApiCell<GraphicsSettings>,
    /// Runtime video settings (resolution / vsync / fullscreen), shared with the
    /// platform layer so a `Video.*` write reaches the surface + window.
    pub video: ApiCell<VideoSettings>,
    /// The current scene file — `Scene.Save()`'s no-path write-back target. Shared
    /// with the platform layer (the editor's current scene file is this cell, #1366).
    pub scene_path: ApiCell<Option<String>>,
    /// The audio engine singleton (#212): the `Audio` namespace drives it
    /// (Play/Stop/SetVolume/PlayAt + master volume). The same maestro the play-mode
    /// systems use, so a scripted play and `play_on_start` share one device + log.
    pub audio: ApiCell<AudioMaestro>,
    /// The seeded gameplay RNG (#443): the `Random` namespace and the sandboxed
    /// `math.random` both draw from it.
    pub random: ApiCell<Random>,
    /// The screen the UI lays out on (#417); resolved against `video` when no
    /// platform reported a game-view size (headless).
    pub screen: ApiCell<ScreenSize>,
    /// Build settings + the quit request (#431): `Application.Quit` raises the flag
    /// the host (player / editor / harness) reads after the tick.
    pub application: ApiCell<Application>,
    /// Frame stats (#433): filled by the dev layer's schedule probe and capture
    /// path, read by the dev-only `Debug.Stats()`. Never read by the sim.
    pub stats: ApiCell<FrameStats>,
    /// Pending script timers and coroutines (#444), behind the `Timer` namespace.
    pub timers: ApiCell<TimerScheduler>,
    /// The UI event system (#420): selection, hover and press state behind the `UI`
    /// namespace's pointer and focus verbs.
    pub event_system: ApiCell<EventSystem>,
    /// The time-sliced environment queries (#2116), behind `EnvironmentQuery.Request`.
    pub queries: ApiCell<QueryQueue>,
}

/// Run `body`, so a namespace test can use `?` on its Lua calls in one place.
#[cfg(test)]
pub(crate) fn attempt<R>(body: impl FnOnce() -> mlua::Result<R>) -> mlua::Result<R> {
    body()
}
