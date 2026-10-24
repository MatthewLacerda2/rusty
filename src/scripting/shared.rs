//! An owned handle to every cell the `api::` surface borrows (#1384).
//!
//! [`ScriptManager::make_ctx`] borrows the manager itself, which suits a caller
//! that holds it for one evaluation. A second VM that drives the same world from
//! the outside (the dev layer's scenario VM) cannot: it holds its API scope for a
//! whole run, while the world it observes ticks, and so borrows the manager
//! mutably, in between. [`ApiCells`] clones the shared `Rc`s once, and
//! [`ApiCells::ctx`] lends them out as the same [`ApiScopedCtx`] the gameplay VM
//! registers against: one surface, one registration, a second caller.

use std::cell::RefCell;
use std::rc::Rc;

use crate::api::ApiScopedCtx;
use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::frame_stats::FrameStats;
use crate::core::input::InputState;
use crate::core::quality::QualityPreset;
use crate::core::random::Random;
use crate::core::storage::Storage;
use crate::core::video::VideoSettings;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::{Camera, Scene};
use crate::time::Time;
use crate::ui::{EventSystem, ScreenSize};

use super::{ConsoleLogs, ScriptManager, TimerScheduler};

/// The shared cells behind one [`ApiScopedCtx`], owned (see the module doc).
pub struct ApiCells {
    scene: Rc<RefCell<Scene>>,
    input: Rc<RefCell<InputState>>,
    nav: Rc<RefCell<NavigationGraph>>,
    camera: Rc<RefCell<Camera>>,
    time: Rc<RefCell<Time>>,
    physics: Rc<RefCell<Option<PhysicsWorld>>>,
    console: Rc<RefCell<ConsoleLogs>>,
    storage: Rc<RefCell<Storage>>,
    quality: Rc<RefCell<QualityPreset>>,
    video: Rc<RefCell<VideoSettings>>,
    scene_path: Rc<RefCell<Option<String>>>,
    audio: Rc<RefCell<AudioMaestro>>,
    random: Rc<RefCell<Random>>,
    screen: Rc<RefCell<ScreenSize>>,
    application: Rc<RefCell<Application>>,
    stats: Rc<RefCell<FrameStats>>,
    timers: Rc<RefCell<TimerScheduler>>,
    event_system: Rc<RefCell<EventSystem>>,
}

impl ApiCells {
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
    /// The cells this runtime's API borrows, owned, for a VM that registers the
    /// same surface over the same world (see the module doc).
    pub fn api_cells(&self) -> ApiCells {
        ApiCells {
            scene: Rc::clone(&self.scene),
            input: Rc::clone(&self.input),
            nav: Rc::clone(&self.nav),
            camera: Rc::clone(&self.camera),
            time: Rc::clone(&self.time),
            physics: Rc::clone(&self.physics),
            console: Rc::clone(&self.console),
            storage: Rc::clone(&self.storage),
            quality: Rc::clone(&self.quality),
            video: Rc::clone(&self.video),
            scene_path: Rc::clone(&self.scene_path),
            audio: Rc::clone(&self.audio),
            random: Rc::clone(&self.random),
            screen: Rc::clone(&self.screen),
            application: Rc::clone(&self.application),
            stats: Rc::clone(&self.stats),
            timers: Rc::clone(&self.timers),
            event_system: Rc::clone(&self.event_system),
        }
    }
}
