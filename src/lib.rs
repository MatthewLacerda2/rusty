pub use rusty_ai as ai;
pub use rusty_app as app;
pub use rusty_asset as asset;
pub use rusty_audio as audio;
pub use rusty_components as components;
pub use rusty_core as core;
pub use rusty_ecs as ecs;
pub use rusty_edit as edit;
pub use rusty_font as font;
pub use rusty_lighting as lighting;
pub use rusty_logging as logging;
pub use rusty_navigation as navigation;
pub use rusty_physics as physics;
pub use rusty_preview as preview;
pub use rusty_procgen as procgen;
pub use rusty_render as render;
pub use rusty_scene as scene;
pub use rusty_script as script;
pub use rusty_shadergen as shadergen;
pub use rusty_shell as shell;
pub use rusty_time as time;
pub use rusty_ui as ui;
pub use rusty_zimmer as zimmer;

#[cfg(feature = "dev")]
pub use rusty_dev as dev;
#[cfg(feature = "editor")]
pub use rusty_editor as editor;

/// Start a binary: the logger, then, in a `dev` build, the engine-content check
/// (`build.rs`): one warning when the `engine/` folder on disk is not the one this
/// binary was built with. Every binary calls it first (the `binary-init` lint).
pub fn init() {
    logging::init();
    #[cfg(feature = "dev")]
    if let Some(built) = option_env!("RUSTY_ENGINE_CONTENT") {
        core::project::engine_content::register_built(built);
    }
}
