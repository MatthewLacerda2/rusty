//! The headless bake entry points: Generate Lighting (every bake a scene has, in
//! order), the one-button lighting bake, the lightmap bake, and the probe and
//! reflection-probe bakes, the occlusion culling's PVS (`occlusion`, #1714), plus
//! the log a script's bake reports its progress to
//! (`progress`, #1667), and the navmesh bake (`navmesh`, #1887), which no
//! lighting step runs. Every bake writes a report of what it measured and found
//! beside the scene (`report`, #1980).

pub mod denoise;
pub mod generate_lighting;
pub mod lighting;
pub mod lightmap;
pub mod navmesh;
pub mod occlusion;
pub mod probe;
pub mod progress;
pub mod reflection;
pub mod report;
pub mod steps;

#[cfg(test)]
mod lightmap_cost_tests;
#[cfg(test)]
mod navmesh_cost_tests;
#[cfg(test)]
mod open_world_tests;
