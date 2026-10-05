//! The editor's dev-build-only state, one field on `EditorUi` (compiled out with
//! the `dev` feature).

/// Editor state that exists only in dev builds.
#[derive(Default)]
pub struct DevEditorState {
    /// Live Lua REPL input line. The editor only collects the submitted text here;
    /// the front-end drains `pending_repl` and runs it through the single
    /// `dev::console` evaluator against the live runtime.
    pub repl_input: crate::dev::console::ReplInput,
    /// A line the user submitted this frame, awaiting evaluation by the front-end.
    pub pending_repl: Option<String>,
    /// The lightmap bake running in the background, if any (#808).
    pub lightmap_bake: Option<super::inspector::assets::scene::bake::ActiveBake>,
    /// The settings the "Bake Lightmaps" button bakes with; the card edits
    /// `directional` (#810). Editor-session state until the scene owns its lighting
    /// settings (#832).
    pub lightmap_settings: crate::scene::lighting::lightmap::BakeSettings,
}
