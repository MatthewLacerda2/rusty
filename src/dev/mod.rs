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
//!   console     — log sink + live Lua REPL (call the API live, windowed or headless)
//!   harness     — headless deterministic runner: Step / StepUntil / results.json
//!   scenario    — loads and runs a `.lua` scenario, captures observations
//!   screenshot  — offscreen wgpu render -> PNG (the agent's "eyes")
//!   botplayer   — pattern notes: a normal script tagged dev-only that drives Input
//!
//! Status: SCAFFOLD — structure only; not yet implemented.
