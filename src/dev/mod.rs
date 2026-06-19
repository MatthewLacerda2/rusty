//! src/dev/ — THE AGENTIC LAYER (dev-only, `#[cfg(feature = "dev")]`)
//!
//! This is the uncharted part of the engine — it has no Unity equivalent.
//! Everything here exists so a coding agent (or you, live) can DRIVE and OBSERVE
//! the game without a human watching a window. The whole module is compiled out of
//! shipped builds: with the `dev` feature off, none of it links, and the Lua
//! `Debug`/`Harness` API tables are never registered — so a ship build's dev/bot
//! scripts simply have nothing to bind against (the Unity "stripped in release"
//! behaviour).
//!
//! Submodules:
//!   session     — long-lived edit-mode host + command channel (the keystone, #177)
//!   harness     — headless deterministic runner: Step / StepUntil / results.json
//!   scenario    — loads and runs a `.lua` scenario, captures observations
//!   bridge      — Lua bindings for the scenario VM (the control surface)
//!   botplayer   — bot-player pattern notes + helper to attach a bot to the Player
//!   demo_scene  — headless demo scene builder (mirrors the windowed scene)
//!   screenshot  — offscreen render -> PNG (the GPU "eyes"); skips if no adapter
//!   snapshot    — world -> JSON observation
//!
//! Status: harness + scenario runner implemented (issue #3); offscreen screenshot
//! implemented (issue #7); bot-player example implemented (issue #10).

pub mod botplayer;
pub mod bridge;
pub mod console;
pub mod demo_scene;
pub mod harness;
pub mod scenario;
pub mod screenshot;
pub mod session;
pub mod snapshot;
