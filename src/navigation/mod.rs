//! Height-field navigation: a per-cell walkable surface (XZ grid + parallel
//! surface-height field, #130) baked from static colliders, A\* search over it,
//! and the per-frame agent steering tick. Split into focused submodules —
//! `grid` (the `NavigationGraph` model + world<->grid/height conversions),
//! `bake` (deriving walkability + the height field from colliders, incl. the
//! agent-radius erosion), `headroom` (the agent-height clearance pass that carves
//! low-overhang cells, #278), `astar` (the height/step-aware A\* search and path
//! queries), and `agents` (cached-path planning + the steering tick) — all hanging
//! off the single re-exported [`NavigationGraph`] type. Agents follow the real baked
//! `y` of the surface (ramps, stairs, multi-level terrain), not a preserved-constant
//! height.

// Panic-free sim core (#195): bare `.unwrap()` is denied here (use `?` or a
// documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

mod agents;
mod astar;
#[cfg(test)]
mod astar_tests;
mod bake;
#[cfg(test)]
mod bake_tests;
#[cfg(test)]
mod erosion_tests;
mod grid;
mod headroom;
#[cfg(test)]
mod headroom_tests;
mod settings;

pub use grid::NavigationGraph;
pub use settings::{NavMeshSettings, DEFAULT_AGENT_HEIGHT, DEFAULT_AGENT_RADIUS};
