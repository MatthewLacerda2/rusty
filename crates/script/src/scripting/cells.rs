use std::cell::RefCell;
use std::rc::Rc;

use rusty_audio::AudioMaestro;
use rusty_core::application::Application;
use rusty_core::frame_stats::FrameStats;
use rusty_core::settings::video::VideoSettings;
use rusty_core::settings::GraphicsSettings;
use rusty_core::storage::Storage;
use rusty_physics::PhysicsWorld;
use rusty_ui::{EventSystem, ScreenSize};

use super::ScriptManager;

impl ScriptManager {
    pub fn set_audio_cell(&mut self, audio: Rc<RefCell<AudioMaestro>>) {
        self.cells.audio = audio;
        self.reattach_cells();
    }

    /// Handle to the live rapier world's cell: the manager owns it from construction
    /// on, `Resources` fills it on Play and empties it on Stop, and `init_runtime`
    /// reads the same cell, so [`ScriptManager::api_cells`] can clone it once.
    pub fn physics_cell(&self) -> Rc<RefCell<Option<PhysicsWorld>>> {
        Rc::clone(&self.cells.physics)
    }

    pub fn set_screen_cell(&mut self, screen: Rc<RefCell<ScreenSize>>) {
        self.cells.screen = screen;
        self.reattach_cells();
    }

    pub fn event_system_cell(&self) -> Rc<RefCell<EventSystem>> {
        Rc::clone(&self.cells.event_system)
    }

    pub fn set_application_cell(&mut self, application: Rc<RefCell<Application>>) {
        self.cells.application = application;
        self.reattach_cells();
    }

    pub fn scene_path_cell(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.cells.scene_path)
    }

    pub fn set_storage(&mut self, storage: Rc<RefCell<Storage>>) {
        self.cells.storage = storage;
        self.reattach_cells();
    }

    pub fn graphics_cell(&self) -> Rc<RefCell<GraphicsSettings>> {
        Rc::clone(&self.cells.graphics)
    }

    pub fn video_cell(&self) -> Rc<RefCell<VideoSettings>> {
        Rc::clone(&self.cells.video)
    }

    pub fn stats_cell(&self) -> Rc<RefCell<FrameStats>> {
        Rc::clone(&self.cells.stats)
    }

    pub fn live_script_count(&self) -> usize {
        self.entity_scripts.len()
    }
}
