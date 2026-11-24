//! The editor's dev-build-only state, one field on `EditorUi` (compiled out with
//! the `dev` feature).

/// Editor state that exists only in dev builds.
#[derive(Default)]
pub struct DevEditorState {
    /// Live Lua REPL input line. The editor only collects the submitted text here;
    /// the front-end drains `pending_repl` and runs it through the single
    /// `dev::console` evaluator against the live runtime.
    pub repl_input: rusty_dev::console::ReplInput,
    /// A line the user submitted this frame, awaiting evaluation by the front-end.
    pub pending_repl: Option<String>,
    /// Never saved: `Storage` is the game's store, not the editor's.
    pub show_play_console: bool,
    /// The demo open on the Game view's timeline, if any.
    pub demo: Option<super::demo::DemoView>,
    /// The Generate Lighting running in the background, if any. Its
    /// settings are the scene's own (`Scene::lighting_settings`).
    pub lightmap_bake: Option<super::inspector::assets::scene::bake::ActiveBake>,
    pub bake_queue: super::inspector::assets::scene::bake_queue::QueueState,
}

#[cfg(test)]
#[path = "dev_state_tests.rs"]
mod tests;
