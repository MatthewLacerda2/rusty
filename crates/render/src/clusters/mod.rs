mod baked;
mod bin;
mod cone;
mod footprint;
mod gpu;
mod grid;
mod lights;
mod lists;
mod unbaked;

pub(crate) use baked::BakedLamps;
pub(crate) use bin::{bin, shadow_requests};
pub(crate) use gpu::ClusterBuffers;
pub(crate) use grid::ClusterGrid;
pub(crate) use lights::{local_lights, LocalLight};
pub(crate) use unbaked::{drawn_mode, UnbakedNote};

pub(crate) const GRID: [u32; 3] = [16, 9, 24];

pub(crate) const CLUSTER_COUNT: usize = (GRID[0] * GRID[1] * GRID[2]) as usize;

pub(crate) const MAX_VISIBLE_LIGHTS: usize = 64;

pub(crate) const MAX_CLUSTER_LIGHTS: usize = 32;

#[cfg(test)]
#[path = "bin_tests.rs"]
mod bin_tests;
#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;
#[cfg(test)]
#[path = "cone_budget_tests.rs"]
mod cone_budget_tests;
#[cfg(test)]
#[path = "cone_tests.rs"]
mod cone_tests;
#[cfg(test)]
#[path = "falloff_gpu_tests.rs"]
mod falloff_gpu_tests;
#[cfg(test)]
#[path = "gpu_tests.rs"]
mod gpu_tests;
#[cfg(test)]
#[path = "lights_tests.rs"]
mod lights_tests;
#[cfg(test)]
#[path = "score_tests.rs"]
mod score_tests;
#[cfg(test)]
#[path = "spheres_tests.rs"]
mod spheres_tests;
