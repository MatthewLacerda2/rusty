//! crates/lighting/src/occlusion/ — the baked occlusion culling's bake (#1714): the
//! potentially visible set a scene holds (`rusty_lighting::occlusion`, whose
//! module doc says what a cell holds and how it is sampled), baked from the static
//! scene. `gather` reads what the bake is against (each object's [`fingerprint`]),
//! `sample` casts the seeded rays and proves the ones it can skip, `bake` drives it
//! and dilates the cells, watched and stopped through `progress`.

mod bake;
mod gather;
mod progress;
mod sample;
mod unsaved;

pub use bake::{
    bake_occlusion, gather_occlusion, OcclusionBakeReport, OcclusionInput, OCCLUSION_SEED,
};
pub use gather::{fingerprint, Fingerprints};
pub use progress::OcclusionObserver;
pub use progress::OcclusionProgress;
pub use sample::{MAX_TARGETS, TARGET_SPACING, VOXEL};
pub use unsaved::unsaved_materials;

#[cfg(test)]
mod bake_tests;
// Test hooks: the levels other crates' bake, render and cost tests build.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
#[cfg(test)]
mod carry_tests;
#[cfg(test)]
mod dense_tests;
#[cfg(test)]
mod file_tests;
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
mod unsaved_tests;
#[cfg(test)]
mod window_tests;
