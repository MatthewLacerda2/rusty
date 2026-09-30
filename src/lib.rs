//! src/lib.rs — the `rusty` library crate.
//!
//! Exposes the engine modules so every binary shares one simulation: the editor
//! (`main.rs`), the standalone player (`bin/player.rs`), and the dev-only headless
//! tools (`bin/play.rs`, `bin/session.rs`, …). Two Cargo features strip layers from a
//! build: `dev` (the harness, console/REPL, `Debug.*`) and `editor` (default-on: the
//! egui editor, `src/editor` + `shell::editor`). A shipped game is the `player` binary
//! built with `--no-default-features` — neither layer.

pub mod api;
pub mod app;
pub mod asset;
pub mod audio;
pub mod components;
pub mod core;
pub mod ecs;
#[cfg(feature = "editor")]
pub mod editor;
pub mod navigation;
pub mod physics;
pub mod preview;
pub mod procgen;
pub mod render;
pub mod scene;
pub mod scripting;
pub mod shadergen;
pub mod shell;
pub mod soundgen;
pub mod time;
pub mod ui;

#[cfg(feature = "dev")]
pub mod dev;
