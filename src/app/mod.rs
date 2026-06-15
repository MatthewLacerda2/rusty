//! src/app/ — the application spine: a tickable GameWorld decoupled from winit/wgpu.
//!
//! Phase 0 of the migration: the simulation that used to live inline in main.rs's
//! event loop now lives in `GameWorld::tick`, which knows nothing about the window
//! or the GPU. This is what makes headless play possible. The full Schedule/System
//! registry (the other scaffold files here) arrives in Phase 1 with hecs.

pub mod game;
mod particles;
mod play;

pub use game::{GameWorld, PlayTransition};
