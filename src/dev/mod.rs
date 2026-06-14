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
//!   harness     — headless deterministic runner: Step / StepUntil / results.json
//!   scenario    — loads and runs a `.lua` scenario, captures observations
//!   bridge      — Lua bindings for the scenario VM (the control surface)
//!   demo_scene  — headless demo scene builder (mirrors the windowed scene)
//!   snapshot    — world -> JSON observation
//!
//! Status: harness + scenario runner implemented (issue #3). screenshot/botplayer
//! (the GPU "eyes" + bot pattern notes) remain scaffolds for a later phase.

pub mod bridge;
pub mod demo_scene;
pub mod harness;
pub mod scenario;
pub mod snapshot;
