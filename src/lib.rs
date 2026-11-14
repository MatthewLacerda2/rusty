//! src/lib.rs — the `rusty` library crate.
//!
//! Exposes the engine modules so every binary shares one simulation: the editor
//! (`main.rs`), the standalone player (`bin/player.rs`), and the dev-only headless
//! tools (`bin/play.rs`, `bin/session.rs`, …). Two Cargo features strip layers from a
//! build: `dev` (the harness, console/REPL, `Debug.*`) and `editor` (default-on: the
//! egui editor, `src/editor` + `shell::editor`). A shipped game is the `player` binary
//! built with `--no-default-features` — neither layer.

pub mod app;
#[cfg(feature = "editor")]
pub mod editor;
pub mod logging;
pub mod render;
pub mod shell;

// The modules already moved into crates of their own (crates/CLAUDE.md), under their
// old names, so every `crate::core::…` path here and `rusty::core::…` path in
// `tests/` keeps compiling.
pub use rusty_ai as ai;
pub use rusty_asset as asset;
pub use rusty_audio as audio;
pub use rusty_components as components;
pub use rusty_core as core;
pub use rusty_ecs as ecs;
pub use rusty_navigation as navigation;
pub use rusty_physics as physics;
pub use rusty_preview as preview;
pub use rusty_procgen as procgen;
pub use rusty_scene as scene;
pub use rusty_script as script;
pub use rusty_shadergen as shadergen;
pub use rusty_time as time;
pub use rusty_ui as ui;

#[cfg(feature = "dev")]
pub mod dev;

#[cfg(test)]
mod test_path;
#[cfg(test)]
mod test_temp;
