//! What one cell of the occlusion bake sees (#1714): the static scene ready to
//! trace (`baking`), the points a ray is aimed at (`targets`), and the proofs that
//! spare most rays: one occluder face (`shaft`), the faces on one plane fused
//! (`sheets`), the faces each sampling cube last met (`eye`), tried by object,
//! cluster (`clusters`) and point (`sight`).

mod baking;
mod clusters;
mod eye;
mod shaft;
mod sheets;
mod sight;
mod targets;

pub(super) use baking::Baking;
#[cfg(test)]
pub(super) use targets::target_points;
pub use targets::{MAX_TARGETS, TARGET_SPACING, VOXEL};

#[cfg(test)]
mod sheets_tests;
