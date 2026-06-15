//! src/lib.rs — the `rusty` library crate.
//!
//! Exposes the engine modules so both the windowed binary (`main.rs`) and the
//! headless `play` binary (`bin/play.rs`) share one simulation. The `dev` tree is
//! gated behind the `dev` Cargo feature and stripped from ship builds.

pub mod api;
pub mod app;
pub mod components;
pub mod core;
pub mod ecs;
pub mod editor;
pub mod navigation;
pub mod physics;
pub mod render;
pub mod scene;
pub mod scripting;
pub mod time;

#[cfg(feature = "dev")]
pub mod dev;
