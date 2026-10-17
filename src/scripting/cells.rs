//! Shared-cell plumbing for [`ScriptManager`]: the `set_*` injectors `Resources` uses
//! to hand the runtime the very cells it holds, and the `*_cell` accessors the app and
//! platform layer read and write through. Split from `manager` so the runtime's own
//! lifecycle stays readable; every cell here is one an `api::` namespace borrows.

use std::cell::RefCell;
use std::rc::Rc;

use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::frame_stats::FrameStats;
use crate::core::quality::QualityPreset;
use crate::core::storage::Storage;
use crate::core::video::VideoSettings;
use crate::ui::{EventSystem, ScreenSize};

use super::ScriptManager;

impl ScriptManager {
    /// Inject the shared audio maestro. `Resources` calls this so the script runtime
    /// and the app drive the same `AudioMaestro` (the `Audio` namespace + the
    /// play-mode systems must agree on one device/log).
    pub fn set_audio_cell(&mut self, audio: Rc<RefCell<AudioMaestro>>) {
        self.audio = audio;
    }

    /// Inject the shared UI screen-size cell (#417). `Resources` calls this so the
    /// `UI` namespace and the layout system read the same screen.
    pub fn set_screen_cell(&mut self, screen: Rc<RefCell<ScreenSize>>) {
        self.screen = screen;
    }

    /// Handle to the UI event system (#420), so `Resources` drives the same state
    /// the `UI` namespace reads.
    pub fn event_system_cell(&self) -> Rc<RefCell<EventSystem>> {
        Rc::clone(&self.event_system)
    }

    /// Inject the shared `Application` cell (build settings + quit request), so the
    /// `Application` namespace and the host that acts on `Quit` agree.
    pub fn set_application_cell(&mut self, application: Rc<RefCell<Application>>) {
        self.application = application;
    }

    /// Handle to the shared play-state cell, so `GameWorld::set_playing` can keep
    /// it in step with the live play-mode flag for `Debug.Snapshot`.
    pub fn play_state_cell(&self) -> Rc<RefCell<bool>> {
        Rc::clone(&self.is_playing)
    }

    /// Handle to the shared scene-path cell, so the platform layer can read/sync
    /// the current scene file with the editor's `current_scene_path`.
    pub fn scene_path_cell(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.scene_path)
    }

    /// Inject the shared persistent store. `Resources` calls this so the script
    /// runtime, the console REPL and the app all read/write the same `Storage`.
    pub fn set_storage(&mut self, storage: Rc<RefCell<Storage>>) {
        self.storage = storage;
    }

    /// Handle to the shared quality-preset cell, so the platform layer can read a
    /// script-driven tier change and apply it to the renderer.
    pub fn quality_cell(&self) -> Rc<RefCell<QualityPreset>> {
        Rc::clone(&self.quality)
    }

    /// Handle to the shared video-settings cell, so the platform layer can read a
    /// script-driven resolution / vsync / fullscreen change and apply it.
    pub fn video_cell(&self) -> Rc<RefCell<VideoSettings>> {
        Rc::clone(&self.video)
    }

    /// Handle to the frame-stats cell (#433) `Debug.Stats()` reads. The dev layer's
    /// schedule probe and the capture path write into it; the sim never reads it.
    pub fn stats_cell(&self) -> Rc<RefCell<FrameStats>> {
        Rc::clone(&self.stats)
    }

    /// Live script instances (one per attached script that loaded) — a frame-stats
    /// world counter.
    pub fn live_script_count(&self) -> usize {
        self.entity_scripts.len()
    }
}
