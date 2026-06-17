//! Grid-based navigation: a flat XZ walkability grid baked from static colliders,
//! A\* search over it, and the per-frame agent steering tick. Split into focused
//! submodules — [`grid`] (the `NavigationGraph` model + bake), [`astar`] (the A\*
//! search and path queries), and [`agents`] (cached-path planning + the steering
//! tick) — all hanging off the single re-exported [`NavigationGraph`] type.

mod agents;
mod astar;
mod grid;

pub use grid::NavigationGraph;
