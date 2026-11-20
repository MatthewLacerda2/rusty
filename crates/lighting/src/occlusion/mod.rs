//! crates/lighting/src/occlusion/ — the baked occlusion culling's bake (#1714): the
//! potentially visible set a scene holds (`rusty_scene::lighting::occlusion`, whose
//! module doc says what a cell holds and how it is sampled), baked from the static
//! scene. `gather` (the base's, `rusty_scene::lighting::occlusion::gather`) reads what
//! the bake is against (each object's [`fingerprint`]),
//! `reach` says which cells a camera can be in, the only ones sampled (#2328),
//! `sample` casts the seeded rays and proves the ones it can skip, `bake` drives it,
//! watched and stopped through `progress`, and `fold` dilates the cells into the
//! rows they share.

mod bake;
mod fold;
use rusty_scene::lighting::occlusion::gather;
mod progress;
mod reach;
mod sample;
#[cfg(feature = "dev")]
mod sampler;
mod unsaved;

pub use bake::{
    bake_occlusion, current_pvs, gather_occlusion, gather_with, is_current, OcclusionBakeReport,
    OcclusionInput, OCCLUSION_SEED, OCCLUSION_VERSION,
};
pub use gather::{fingerprint, Fingerprints};
pub use progress::OcclusionObserver;
pub use progress::OcclusionProgress;
pub use reach::{Cells, Reach, PLAY_HEIGHT};
pub use sample::{MAX_TARGETS, TARGET_SPACING, VOXEL};
#[cfg(feature = "dev")]
#[doc(hidden)]
pub use sampler::Sampler;
pub use unsaved::unsaved_materials;

#[cfg(test)]
mod bake_tests;
// Test hooks: the levels other crates' bake, render and cost tests build.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
#[cfg(test)]
mod dense_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod kept_tests;
#[cfg(all(test, feature = "dev"))]
mod opening_tests;
#[cfg(test)]
mod print_tests;
#[cfg(test)]
mod sampling_tests;
#[cfg(all(test, feature = "dev"))]
mod tessellated_tests;
#[doc(hidden)]
pub mod test_gap;
#[doc(hidden)]
pub mod test_level;
#[cfg(test)]
mod version_tests;
#[cfg(test)]
mod window_tests;
