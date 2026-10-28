//! src/core/application.rs — `Application`: build settings + the quit request.
//!
//! The standalone player (#431) needs two small pieces of engine-wide state that are
//! neither scene data nor save data:
//!
//! * **[`BuildSettings`]** — Unity's "Build Settings / Player Settings", minimal: which
//!   scene a shipped build boots into, the product name (the player's window title) and
//!   the default window mode. They live in the project's **tracked** `project.rusty`
//!   ([`PROJECT_FILE`](crate::core::project::PROJECT_FILE), #853, under `"build"`), so
//!   they travel with the project in git instead of with a developer's local save. The editor edits them (File → Build Settings) and scripts
//!   through the `Application` namespace — one surface, per the parity rule.
//! * **The project's graphics option defaults** — what a fresh install's player
//!   graphics options start as (#1438), in the same file under `"graphics_options"`.
//!   `Graphics.SetDefaultOption` and the Build Settings window edit them, the same way.
//! * **The project's decal budgets** (#1517), under `"decals"`: read, never written
//!   here, so an edit to the file is what the next Play applies (and Stop restores).
//! * **The quit request** — `Application.Quit()` only *records* that the game asked to
//!   quit. What that means is the host's call: the player closes the window, the editor
//!   stops Play (Unity ignores `Quit` in the editor), and the headless harness ends the
//!   run. The sim never touches the window; it raises a flag the platform layer reads.
//! * **The play request** — every host asks to enter or leave Play here
//!   (`GameWorld::set_playing`: the editor transport, the player, the harness), and so
//!   do the dev-only `Editor.EnterPlay()` / `Editor.ExitPlay()` (#1360, Unity's
//!   `EditorApplication.isPlaying`). A script's request is kept apart from the host's
//!   until `GameWorld` takes it at the next frame boundary, so a script never runs the
//!   transition from inside its own call and the transport, writing its own request
//!   back every frame, never overwrites it. A **standalone** run (the player, the
//!   harness) has no edit mode to return to, and refuses a script's request.
//!
//! ## Persistence
//! Like [`Storage`](crate::core::storage::Storage), the file is read once at a boundary
//! (boot) and an unbound instance ([`Application::new`]) never touches disk — the
//! harness and tests stay reproducible. Unlike `Storage`, a setter on a *bound*
//! instance writes the file straight away: build settings are authoring-time project
//! config, edited rarely and deliberately, never from inside `FixedUpdate`. The player
//! and `play` take the settings opening the project read
//! ([`Opened::build`](crate::core::project::Opened::build)) into an unbound instance,
//! so a shipped game can never rewrite its own build settings.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::project::ProjectFile;
use crate::core::settings::decals::DecalLimits;
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};

/// The startup scene when none is configured: the seeded demo scene. Kept equal to
/// `scene::DEFAULT_SCENE_PATH` (a unit test pins it) without `core` importing `scene`.
pub const DEFAULT_STARTUP_SCENE: &str = "assets/scenes/default.scene";

/// The product name when none is configured.
pub const DEFAULT_PRODUCT_NAME: &str = "rusty game";

crate::core::named_enum! {
    /// How the player's window opens on a first launch. A player who later changes
    /// `Video.SetFullscreen` keeps that choice (it persists through `Storage`); this is
    /// only the default before any such choice exists.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum WindowMode {
        #[default]
        Windowed = "Windowed",
        Fullscreen = "Fullscreen",
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

/// The `Application` resource: the project's build settings plus the game's pending
/// quit request. One per World, shared by the script runtime and the platform layer.
#[derive(Default)]
pub struct Application {
    build: BuildSettings,
    /// The project's default player graphics options (#1438).
    graphics_defaults: GraphicsOptions,
    /// The project's decal budgets (#1517) as last read or set.
    decal_limits: DecalLimits,
    /// The bound file's text the budgets were last parsed from, good or bad: the
    /// same text again is not re-parsed, so its warnings are not repeated.
    decals_read_from: Option<String>,
    /// `Some` once bound to a file via [`Application::open`]: setters then write it.
    path: Option<PathBuf>,
    quit_requested: bool,
    /// Whether the host asked for Play (`GameWorld::set_playing`); the next tick's
    /// transition acts on it.
    host_play: bool,
    /// `Some(playing)` once a script asked to enter or leave Play (#1360).
    play_request: Option<bool>,
    /// A standalone run (the player, the harness, #431): no edit mode to return to.
    standalone: bool,
}

impl Application {
    /// Default settings, bound to no file — the harness and tests.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read the project file at `path` and bind it (the editor), so every setter writes
    /// the settings back into it.
    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let read = ProjectFile::read(&path);
        self.path = Some(path);
        let file = read?.unwrap_or_default();
        (self.build, self.graphics_defaults) = (file.build, file.graphics_options);
        self.decal_limits = file.decals;
        self.decals_read_from = self
            .path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok());
        Ok(())
    }

    /// The project's decal budgets, which entering and leaving Play apply to the
    /// scene. Bound, the file as it stands now, so an edit to `project.rusty` (by
    /// hand or by the agent) reaches the next Play; unbound, the ones
    /// [`set_decal_limits`] gave. A bound file that can't be read or parsed keeps
    /// the last good read, and says so once per content.
    ///
    /// [`set_decal_limits`]: Self::set_decal_limits
    pub fn decal_limits(&mut self) -> DecalLimits {
        let Some(path) = &self.path else {
            return self.decal_limits;
        };
        let why = match std::fs::read_to_string(path) {
            Ok(text) if self.decals_read_from.as_ref() == Some(&text) => return self.decal_limits,
            Ok(text) => {
                let parsed = ProjectFile::from_json(&text);
                self.decals_read_from = Some(text);
                match parsed {
                    Ok(file) => {
                        self.decal_limits = file.decals;
                        return self.decal_limits;
                    }
                    Err(err) => err,
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                format!("{} is missing", path.display())
            }
            Err(err) => format!("Failed to read {}: {err}", path.display()),
        };
        log::warn!("[Project] decals: {why}; keeping the budgets last read");
        self.decal_limits
    }

    /// The decal budgets an unbound run plays with (the player, the harness: what
    /// opening the project read).
    pub fn set_decal_limits(&mut self, limits: DecalLimits) {
        self.decal_limits = limits;
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

    /// The project's default player graphics options.
    pub fn graphics_defaults(&self) -> GraphicsOptions {
        self.graphics_defaults
    }

    /// Replace the default graphics options, writing the bound file (if any) once.
    pub fn set_graphics_defaults(&mut self, defaults: GraphicsOptions) -> Result<(), String> {
        self.graphics_defaults = defaults;
        self.save()
    }

    /// Set one default graphics option (`Graphics.SetDefaultOption`, the Build
    /// Settings window: one op), writing the bound file (if any). The `live`
    /// options follow it wherever the player hasn't chosen, so the editor previews
    /// what a fresh install sees.
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

    /// Write the settings into the bound project file, keeping the rest of it. A
    /// no-op when unbound.
    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut file = ProjectFile::read(path)?.unwrap_or_default();
        file.build = self.build.clone();
        file.graphics_options = self.graphics_defaults;
        file.write(path)
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

    /// Whether the host asked for Play: the state the next transition heads to.
    pub fn host_play(&self) -> bool {
        self.host_play
    }

    /// Record the host's own request (`GameWorld::set_playing`).
    pub fn set_host_play(&mut self, playing: bool) {
        self.host_play = playing;
    }

    /// Where the next transition heads: a script's request not yet taken, else the
    /// host's.
    pub fn play_heading(&self) -> bool {
        self.play_request.unwrap_or(self.host_play)
    }

    /// Ask the host to enter (`true`) or leave (`false`) Play at the next frame
    /// boundary (#1360). A later request before that boundary replaces this one.
    pub fn request_play(&mut self, playing: bool) {
        self.play_request = Some(playing);
    }

    /// Consume the pending play request: the host's frame boundary calls it once.
    pub fn take_play_request(&mut self) -> Option<bool> {
        self.play_request.take()
    }

    /// Whether this is a standalone run, with no edit mode to return to.
    pub fn standalone(&self) -> bool {
        self.standalone
    }

    /// Mark the run standalone, once, at boot (`GameWorld::boot_standalone`).
    pub fn mark_standalone(&mut self) {
        self.standalone = true;
    }
}

#[cfg(test)]
#[path = "application_tests.rs"]
mod application_tests;
