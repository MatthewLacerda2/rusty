//! crates/scene/src/rules/material.rs — the ranges a material's numeric fields hold.
//!
//! The material ops (`authoring::material`) clamp to these, the editor's sliders
//! span them, and the renderer holds a value that skipped them (a scene file) to
//! the same range, so each limit lives here once.

use std::ops::RangeInclusive;

/// The range of the `[0, 1]` factors (metallic, roughness, alpha, alpha cutoff): the
/// setters clamp to it (a non-finite value reads as 0) and the editor's sliders span
/// it, so the limit lives here once, as `rigidbody::set_mass`'s does (#993).
pub const FACTOR_RANGE: RangeInclusive<f32> = 0.0..=1.0;
/// HDRP's range for the three detail scales (albedo, normal, smoothness).
pub const DETAIL_SCALE_RANGE: RangeInclusive<f32> = 0.0..=2.0;
/// Each lane of `tiling` and `offset`.
pub const MAP_COORD_RANGE: RangeInclusive<f32> = -1000.0..=1000.0;
/// `world_size`, in metres per repeat.
pub const WORLD_SIZE_RANGE: RangeInclusive<f32> = 0.01..=100.0;
/// `blend_sharpness`, the power the normal is raised to.
pub const BLEND_SHARPNESS_RANGE: RangeInclusive<f32> = 1.0..=64.0;
/// `height_scale` (#2191): the height map's depth as a fraction of one repeat's width.
pub const HEIGHT_SCALE_RANGE: RangeInclusive<f32> = 0.0..=rusty_components::HEIGHT_SCALE_MAX;
