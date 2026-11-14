//! src/lib.rs — the `rusty` library crate.
//!
//! Holds no code of its own: it re-exports every workspace crate (`crates/<module>/`,
//! crates/CLAUDE.md) under its module's name, so the binaries (`main.rs`, `bin/`)
//! and the integration tests (`tests/`) name one surface, `rusty::core::…`. Two Cargo
//! features strip layers from a build: `dev` (the harness, console/REPL, `Debug.*`)
//! and `editor` (default-on: the egui editor, `rusty-editor` + `shell::editor`). A
//! shipped game is the `player` binary built with `--no-default-features`, neither layer.

pub use rusty_ai as ai;
pub use rusty_app as app;
pub use rusty_asset as asset;
pub use rusty_audio as audio;
pub use rusty_components as components;
pub use rusty_core as core;
pub use rusty_ecs as ecs;
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

#[cfg(feature = "dev")]
pub use rusty_dev as dev;
#[cfg(feature = "editor")]
pub use rusty_editor as editor;
