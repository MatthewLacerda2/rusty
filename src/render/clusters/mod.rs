//! src/render/clusters/ — clustered forward (forward+) lighting (#434).
//!
//! The view frustum is cut into a [`GRID`] of clusters: screen tiles across and up,
//! and depth slices spaced exponentially for a perspective camera (linearly for an
//! orthographic one). Each frame the scene's point and spot lights go into one
//! storage array ([`LocalLight`]); each camera then bins every light's range
//! sphere into the clusters it touches, a spotlight only those its cone may reach
//! (`cone`, #1712), on the CPU ([`bin`]), and uploads a
//! `(offset, count)` per cluster plus the flat list of light indices. A fragment
//! finds its cluster from its world position (`cluster_index` in `common.wgsl`)
//! and shades only that cluster's lights. This is the shape of Unity URP's
//! Forward+ and of most modern forward renderers.
//!
//! **Budget (#834).** A camera shades at most [`MAX_VISIBLE_LIGHTS`] local lights,
//! and one cluster — so one pixel — at most [`MAX_CLUSTER_LIGHTS`]. Past either,
//! the lights that contribute most win: brightness (intensity × colour luminance)
//! with a range falloff, measured over the view for the camera cut and at the
//! cluster for the cluster cut. The rest are counted, never silent: per camera in
//! `RenderCounters::lights_dropped`, per cluster in `cluster_lights_dropped`.
//! Both cuts rank by range sphere, so a spotlight's cone changes which clusters
//! list it, never which lights are kept. Lights outside a camera's frustum are
//! culled before binning, and a kept light that contributes nothing (zero
//! intensity or colour, #1712) lists in no cluster; neither costs a pixel
//! anything (`lights_culled`). Decals share the binner with their own budget.
//!
//! **Consumers.** The forward pass (`shader.wgsl`, and through it every authored
//! surface shader and the transparent pass) and lit particles (`particles.wgsl`)
//! read lights only through the cluster lookup.

mod bin;
mod cone;
mod footprint;
mod gpu;
mod grid;
mod lights;
mod lists;

pub(crate) use bin::{bin, shadow_requests};
pub(crate) use gpu::ClusterBuffers;
pub(crate) use grid::ClusterGrid;
pub(crate) use lights::{local_lights, LocalLight};

/// Clusters across, up and deep. 16×9 tiles match a 16:9 screen with square-ish
/// tiles; 24 exponential slices keep each slice's depth span proportional to its
/// distance (the Doom 2016 / Olsson layout).
pub(crate) const GRID: [u32; 3] = [16, 9, 24];

/// Clusters in the grid.
pub(crate) const CLUSTER_COUNT: usize = (GRID[0] * GRID[1] * GRID[2]) as usize;

/// The most local (point + spot) lights one camera shades (#834). The ones that
/// contribute most to the view win; the rest are reported as dropped. Levels light
/// mostly with baked light plus a handful of realtime ones, so 64 is generous.
pub(crate) const MAX_VISIBLE_LIGHTS: usize = 64;

/// The most local lights one cluster lists, which bounds the per-pixel light loop
/// (#834). The ones that contribute most to the cluster win.
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
#[path = "spheres_tests.rs"]
mod spheres_tests;
