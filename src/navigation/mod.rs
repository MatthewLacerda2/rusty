//! Height-field navigation: a per-cell walkable surface (XZ grid + parallel
//! surface-height field, #130) baked from static colliders, A\* search over it,
//! and the per-frame agent steering tick. Split into focused submodules —
//! `grid` (the `NavigationGraph` model + world<->grid/height conversions),
//! `bake` (deriving walkability + the height field from colliders), `astar`
//! (the height/step-aware A\* search and path queries), and `agents`
//! (cached-path planning + the steering tick) — all hanging off the single
//! re-exported [`NavigationGraph`] type. Agents follow the real baked `y` of the
//! surface (ramps, stairs, multi-level terrain), not a preserved-constant height.

mod agents;
mod astar;
#[cfg(test)]
mod astar_tests;
mod bake;
mod grid;

pub use grid::NavigationGraph;
