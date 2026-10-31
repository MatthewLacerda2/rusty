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
//!   session         — long-lived edit-mode host + command channel (the keystone, #177)
//!   console         — the console: log sink + the live Lua REPL
//!   command_channel — windowed socket command channel: drive a live playtest (#282)
//!   mcp             — MCP stdio server over the same evaluator (Blender-MCP-style, #288)
//!   demo        — every Play session recorded, replayed headless bit for bit (#1581)
//!   harness     — headless deterministic runner: Step / StepUntil / results.json
//!   scenario    — loads and runs a `.lua` scenario, captures observations
//!   bridge      — Lua bindings for the scenario VM (the control surface)
//!   botplayer   — bot-player pattern notes + helper to attach a bot to the Player
//!   capture     — one renderer + view held across N shots (#355), shared by the two below
//!   screenshot  — offscreen render -> PNG (the GPU "eyes"); skips if no adapter
//!   preview     — headless asset preview -> PNG (eyes on *assets*, #353)
//!   snapshot    — world -> JSON observation
//!   bake        — the bakes; Generate Lighting: lightmaps, then probes, then written (#832)
//!   lua_surface — `Debug.*` and the `*.Bake` verbs, installed onto the Lua surface (#737)
//!   stats       — frame stats: the schedule timing probe + render counters (#433)
//!   bench       — the stress scene and report behind `make bench` (#835)
//!   export      — ship a project: runtime files and the import cache, no sources (#1267)
//!   providers   — the boundary paid, networked calls live behind: key, budget, edit-mode only (#383)
//!
//! Status: harness + scenario runner implemented (issue #3); offscreen screenshot
//! implemented (issue #7); bot-player example implemented (issue #10).

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "platform layer, exempt from the sim's clock ban (#757): frame timing and screenshot timeouts measure real time"
)]

pub mod bake;
pub mod bench;
pub mod botplayer;
pub mod bridge;
pub mod capture;
pub mod command_channel;
pub mod console;
pub mod demo;
pub mod export;
pub mod harness;
pub mod lua_surface;
pub mod mcp;
pub mod preview;
pub mod providers;
pub mod scenario;
pub mod screenshot;
pub mod session;
pub mod snapshot;
pub mod stats;
