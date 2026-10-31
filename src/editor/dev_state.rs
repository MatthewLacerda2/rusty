//! The editor's dev-build-only state, one field on `EditorUi` (compiled out with
//! the `dev` feature).

use crate::core::storage::Storage;

/// The `Storage` namespace and key holding the Developer Console choice. `Storage`
/// is the project's own store (`saved/storage.json` in the project root), so the
/// choice is per project and survives Play, Stop and a restart (#1544).
const EDITOR_NAMESPACE: &str = "editor";
const PLAY_CONSOLE_KEY: &str = "show_play_console";

/// Editor state that exists only in dev builds.
#[derive(Default)]
pub struct DevEditorState {
    /// Live Lua REPL input line. The editor only collects the submitted text here;
    /// the front-end drains `pending_repl` and runs it through the single
    /// `dev::console` evaluator against the live runtime.
    pub repl_input: crate::dev::console::ReplInput,
    /// A line the user submitted this frame, awaiting evaluation by the front-end.
    pub pending_repl: Option<String>,
    /// Whether the floating Developer Console shows during Play. Off by default, as
    /// Unity's Console is a window you open; remembered per project (#1544).
    pub show_play_console: bool,
    /// The demo open on the Game view's timeline, if any (#1598).
    pub demo: Option<super::demo::DemoView>,
    /// The Generate Lighting running in the background, if any (#808, #832). Its
    /// settings are the scene's own (`Scene::lighting_settings`).
    pub lightmap_bake: Option<super::inspector::assets::scene::bake::ActiveBake>,
}

impl DevEditorState {
    /// Read the project's saved choices; a missing value is the default (hidden).
    pub fn load_prefs(&mut self, storage: &Storage) {
        self.show_play_console = storage
            .get(EDITOR_NAMESPACE, PLAY_CONSOLE_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    }

    /// Write the choices into `storage` when they changed (the Stop / quit flush puts
    /// them on disk). Returns whether it wrote.
    pub fn store_prefs(&self, storage: &mut Storage) -> bool {
        let saved = storage
            .get(EDITOR_NAMESPACE, PLAY_CONSOLE_KEY)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if saved == self.show_play_console {
            return false;
        }
        storage.set(
            EDITOR_NAMESPACE,
            PLAY_CONSOLE_KEY,
            self.show_play_console.into(),
        );
        true
    }
}

#[cfg(test)]
#[path = "dev_state_tests.rs"]
mod tests;
