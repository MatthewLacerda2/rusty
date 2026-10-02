//! Shared-cell plumbing for [`ScriptManager`]: the `set_*_cell` injectors the app and
//! platform layer use to hand the runtime the very cells they hold, and the `*_cell`
//! accessors they read back through. Split from `manager` so the runtime's own
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

    /// Handle to the shared audio maestro, so the platform layer can inject the real
    /// `KiraBackend` and read the introspection log.
    pub fn audio_cell(&self) -> Rc<RefCell<AudioMaestro>> {
        Rc::clone(&self.audio)
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

    /// Inject the shared scene-path cell — the `Scene.Save()` write-back target.
    /// The platform layer keeps this in sync with `EditorUi.current_scene_path`,
    /// and the headless session sets it from the loaded boot scene, so a scripted
    /// save writes back to the same file the editor would.
    pub fn set_scene_path_cell(&mut self, scene_path: Rc<RefCell<Option<String>>>) {
        self.scene_path = scene_path;
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

    /// Inject the shared quality-preset cell. The platform layer keeps this cell
    /// in sync with the renderer's tier, so `Graphics.SetQuality` from a script
    /// reaches `renderer.set_quality` (which guards the bloom-buffer realloc).
    pub fn set_quality_cell(&mut self, quality: Rc<RefCell<QualityPreset>>) {
        self.quality = quality;
    }

    /// Handle to the shared quality-preset cell, so the platform layer can read a
    /// script-driven tier change and apply it to the renderer.
    pub fn quality_cell(&self) -> Rc<RefCell<QualityPreset>> {
        Rc::clone(&self.quality)
    }

    /// Inject the shared video-settings cell. The platform layer keeps this in sync
    /// with the live surface/window, so `Video.*` writes from a script reach the
    /// wgpu surface (resolution / present mode) and the winit window (fullscreen).
    pub fn set_video_cell(&mut self, video: Rc<RefCell<VideoSettings>>) {
        self.video = video;
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
