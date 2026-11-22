mod bake;
mod fold;
use rusty_scene::lighting::occlusion::gather;
mod progress;
mod reach;
mod report;
mod sample;
#[cfg(feature = "dev")]
mod sampler;
mod unsaved;

pub use bake::{
    bake_occlusion, current_pvs, gather_occlusion, gather_with, is_current, OcclusionInput,
    OCCLUSION_SEED, OCCLUSION_VERSION,
};
pub use gather::{fingerprint, Fingerprints};
pub use progress::OcclusionObserver;
pub use progress::OcclusionProgress;
pub use reach::{play_height, Cells, Reach, JUMP_ALLOWANCE, MIN_PLAY_HEIGHT};
pub use report::OcclusionBakeReport;
pub use sample::{MAX_TARGETS, TARGET_SPACING, VOXEL};
#[cfg(feature = "dev")]
#[doc(hidden)]
/// A test hook: one bake's sampling, for `dev::bake`'s cost tests.
pub use sampler::Sampler;
pub use unsaved::unsaved_materials;

#[cfg(test)]
mod bake_tests;
#[cfg(feature = "dev")]
#[doc(hidden)]
/// A test hook: the bench levels other crates' cost tests build.
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
/// Test hooks: levels other crates' bake tests build.
#[doc(hidden)]
pub mod test_gap;
/// Test hooks, as `test_gap`.
#[doc(hidden)]
pub mod test_level;
#[cfg(test)]
mod version_tests;
#[cfg(test)]
mod window_tests;
