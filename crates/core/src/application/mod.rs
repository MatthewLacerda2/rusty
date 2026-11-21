//! Engine-wide state that is neither scene nor save data (most of it from
//! `project.rusty`), plus the editor's mailboxes and the quit and play requests.
//!
//! `Quit()` only records; what it means is the host's call (Unity ignores it in the
//! editor). A script's play request is kept apart from the host's so the transport,
//! rewriting its own every frame, never overwrites it.
//!
//! An unbound instance never touches disk; a bound one's setters write the file.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::project::{ProjectFile, ProjectSettings};
use crate::settings::audio::AudioSettings;
use crate::settings::decals::DecalLimits;
use crate::settings::demos::DemoSettings;
use crate::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::settings::logs::LogSettings;
use crate::settings::quality::QualityPresets;
use crate::settings::render_scale::ProjectGraphics;

mod demo;
mod game_view;
mod gizmos;
mod host;
mod loading;
mod logs;
mod quality;
mod render_scale;
mod reread;

pub use loading::LoadingPriority;

pub use demo::DemoRequest;
pub use game_view::{letterbox, GameViewSetting, GameViewSize, MAX_GAME_VIEW_SIDE};
pub use gizmos::{Gizmo, GizmoRow, SceneGizmos};

/// The startup scene when none is configured: the seeded demo scene. Kept equal to
/// `edit::DEFAULT_SCENE_PATH` (a unit test pins it) without `core` importing `edit`.
pub const DEFAULT_STARTUP_SCENE: &str = "assets/scenes/default.scene";

pub const DEFAULT_PRODUCT_NAME: &str = "rusty game";

crate::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum WindowMode {
        #[default]
        Windowed = "Windowed",
        Fullscreen = "Fullscreen",
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildSettings {
    pub startup_scene: String,
    pub product_name: String,
    pub window_mode: WindowMode,
}

impl Default for BuildSettings {
    fn default() -> Self {
        Self {
            startup_scene: DEFAULT_STARTUP_SCENE.to_string(),
            product_name: DEFAULT_PRODUCT_NAME.to_string(),
            window_mode: WindowMode::default(),
        }
    }
}

#[derive(Default)]
pub struct Application {
    build: BuildSettings,
    graphics_defaults: GraphicsOptions,
    graphics: ProjectGraphics,
    quality: QualityPresets,
    quality_selected: Option<String>,
    decal_limits: DecalLimits,
    demos: DemoSettings,
    audio: AudioSettings,
    logs: LogSettings,
    read_from: Option<String>,
    path: Option<PathBuf>,
    quit_requested: bool,
    host_play: bool,
    play_request: Option<bool>,
    standalone: bool,
    bake_host: bool,
    demo: demo::DemoDesk,
    gizmos: gizmos::GizmoDesk,
    game_view: game_view::GameViewDesk,
    loading_priority: LoadingPriority,
}

impl Application {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        // lint: allow(stored-format) `project.rusty`'s format is `core::project::file`'s
        let read = match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(format!("Failed to read {}: {err}", path.display())),
        };
        self.path = Some(path);
        let text = read?;
        let parsed = text.as_deref().map(ProjectFile::from_json);
        self.read_from = text;
        let file = parsed.transpose()?.unwrap_or_default();
        (self.build, self.graphics_defaults) = (file.build, file.graphics_options);
        self.graphics = file.graphics;
        self.quality = file.quality;
        self.decal_limits = file.decals;
        (self.demos, self.audio, self.logs) = (file.demos, file.audio, file.logs);
        Ok(())
    }

    pub fn run_with(&mut self, settings: ProjectSettings) {
        self.build = settings.build;
        self.graphics_defaults = settings.graphics_options;
        self.graphics = settings.graphics;
        self.quality = settings.quality;
        self.decal_limits = settings.decals;
        (self.demos, self.audio, self.logs) = (settings.demos, settings.audio, settings.logs);
    }

    pub fn settings(&self) -> ProjectSettings {
        ProjectSettings {
            build: self.build.clone(),
            graphics_options: self.graphics_defaults,
            graphics: self.graphics,
            quality: self.quality.clone(),
            decals: self.decal_limits,
            demos: self.demos,
            audio: self.audio,
            logs: self.logs,
        }
    }

    pub fn build(&self) -> &BuildSettings {
        &self.build
    }

    pub fn set_build(&mut self, build: BuildSettings) -> Result<(), String> {
        self.build = build;
        self.save()
    }

    pub fn update(&mut self, edit: impl FnOnce(&mut BuildSettings)) -> Result<(), String> {
        edit(&mut self.build);
        self.save()
    }

    pub fn graphics_defaults(&self) -> GraphicsOptions {
        self.graphics_defaults
    }

    pub fn set_graphics_defaults(&mut self, defaults: GraphicsOptions) -> Result<(), String> {
        self.graphics_defaults = defaults;
        self.save()
    }

    pub fn set_default_option(
        &mut self,
        option: GraphicsOption,
        on: bool,
        live: &mut GraphicsOptions,
    ) -> Result<(), String> {
        self.graphics_defaults.set(option, on);
        live.rebase(self.graphics_defaults);
        self.save()
    }

    fn save(&mut self) -> Result<(), String> {
        self.reread();
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut file = ProjectFile::read(path)?.unwrap_or_default();
        file.build = self.build.clone();
        file.graphics_options = self.graphics_defaults;
        file.graphics = self.graphics;
        file.quality = self.quality.clone();
        (file.audio, file.logs) = (self.audio, self.logs);
        file.write(path)
    }
}

#[cfg(test)]
mod tests;
