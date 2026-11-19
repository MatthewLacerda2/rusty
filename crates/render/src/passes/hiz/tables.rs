//! The CPU tables a camera's cull refills each frame (#1963).

use glam::Vec3;

/// The CPU tables a cull fills each frame, kept so a frame allocates nothing once
/// they reached the scene's size.
#[derive(Default)]
pub(crate) struct Tables {
    /// Each instance's object slot.
    pub(crate) slots: Vec<u32>,
    /// The tested bounds by object slot.
    pub(crate) by_slot: Vec<Option<(Vec3, Vec3)>>,
    /// Each instance's bounds, as `hiz.wgsl`'s `Bounds`.
    pub(crate) bounds: Vec<[f32; 8]>,
}
