//! src/core/application.rs — `Application`: build settings + the quit request.
//!
//! The standalone player (#431) needs two small pieces of engine-wide state that are
//! neither scene data nor save data:
//!
//! * **[`BuildSettings`]** — Unity's "Build Settings / Player Settings", minimal: which
//!   scene a shipped build boots into, the product name (the player's window title) and
//!   the default window mode. They live in a small **tracked** project file,
//!   [`BUILD_SETTINGS_PATH`], so they travel with the project in git instead of with a
//!   developer's local save. The editor edits them (File → Build Settings) and scripts
//!   through the `Application` namespace — one surface, per the parity rule.
//! * **The quit request** — `Application.Quit()` only *records* that the game asked to
//!   quit. What that means is the host's call: the player closes the window, the editor
//!   stops Play (Unity ignores `Quit` in the editor), and the headless harness ends the
//!   run. The sim never touches the window; it raises a flag the platform layer reads.
//!
//! ## Persistence
//! Like [`Storage`](crate::core::storage::Storage), the file is read once at a boundary
//! (boot) and an unbound instance ([`Application::new`]) never touches disk — the
//! harness and tests stay reproducible. Unlike `Storage`, a setter on a *bound*
//! instance writes the file straight away: build settings are authoring-time project
//! config, edited rarely and deliberately, never from inside `FixedUpdate`. The player
//! reads the file without binding it ([`Application::load`]), so a shipped game can
//! never rewrite its own build settings.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The tracked project file the build settings live in.
pub const BUILD_SETTINGS_PATH: &str = "build_settings.json";

/// The startup scene when none is configured: the seeded demo scene. Kept equal to
/// `scene::DEFAULT_SCENE_PATH` (a unit test pins it) without `core` importing `scene`.
pub const DEFAULT_STARTUP_SCENE: &str = "assets/scenes/default.scene";

/// The product name when none is configured.
pub const DEFAULT_PRODUCT_NAME: &str = "rusty game";

/// How the player's window opens on a first launch. A player who later changes
/// `Video.SetFullscreen` keeps that choice (it persists through `Storage`); this is
/// only the default before any such choice exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowMode {
    #[default]
    Windowed,
    Fullscreen,
}

impl WindowMode {
    /// The canonical name, as the `Application` namespace speaks it.
    pub fn name(self) -> &'static str {
        match self {
            WindowMode::Windowed => "Windowed",
            WindowMode::Fullscreen => "Fullscreen",
        }
    }

    /// `"Windowed"` / `"Fullscreen"` (case-insensitive) → mode; anything else `None`.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "windowed" => Some(WindowMode::Windowed),
            "fullscreen" => Some(WindowMode::Fullscreen),
            _ => None,
        }
    }
}

/// Project-level build configuration (Unity's "Scenes In Build", minimal). Every field
/// is `#[serde(default)]`, so a partial file keeps the defaults for what it omits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildSettings {
    /// The scene the player loads and enters Play in on launch.
    pub startup_scene: String,
    /// The shipped game's name — the player's window title.
    pub product_name: String,
    /// The player's window mode on a first launch.
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

impl BuildSettings {
    /// Parse settings from the file's JSON text. Malformed JSON is an error; missing
    /// fields fall back to the defaults.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("Failed to parse build settings: {e}"))
    }

    /// The settings as the pretty JSON the tracked file holds (with a trailing newline).
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Read settings from `path`. A missing file is the normal case for a project that
    /// never configured a build, so it yields the defaults; an unreadable or malformed
    /// file is an error.
    pub fn read(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read build settings: {e}"))?;
        Self::from_json(&text)
    }
}

/// The `Application` resource: the project's build settings plus the game's pending
/// quit request. One per World, shared by the script runtime and the platform layer.
#[derive(Default)]
pub struct Application {
    build: BuildSettings,
    /// `Some` once bound to a file via [`Application::open`]: setters then write it.
    path: Option<PathBuf>,
    quit_requested: bool,
}

impl Application {
    /// Default settings, bound to no file — the harness and tests.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read `path` without binding it (the player): setters change the in-memory
    /// settings only, so a shipped game never rewrites its build settings.
    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        self.build = BuildSettings::read(path.as_ref())?;
        Ok(())
    }

    /// Read `path` and bind it (the editor), so every setter writes it back.
    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let read = BuildSettings::read(&path);
        self.path = Some(path);
        self.build = read?;
        Ok(())
    }

    /// The current build settings.
    pub fn build(&self) -> &BuildSettings {
        &self.build
    }

    /// Replace the build settings, writing the bound file (if any) once.
    pub fn set_build(&mut self, build: BuildSettings) -> Result<(), String> {
        self.build = build;
        self.save()
    }

    /// Edit the build settings in place, then write the bound file (if any) once.
    pub fn update(&mut self, edit: impl FnOnce(&mut BuildSettings)) -> Result<(), String> {
        edit(&mut self.build);
        self.save()
    }

    /// Write the settings to the bound file. A no-op when unbound.
    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, self.build.to_json())
            .map_err(|e| format!("Failed to write build settings: {e}"))
    }

    /// Record that the game asked to quit (`Application.Quit()`).
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Whether a quit is pending, without clearing it (the harness keeps the run ended).
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Consume a pending quit request: `true` exactly once per `Quit()` call.
    pub fn take_quit_request(&mut self) -> bool {
        std::mem::take(&mut self.quit_requested)
    }
}

#[cfg(test)]
#[path = "application_tests.rs"]
mod application_tests;
