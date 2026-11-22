//! A material's numeric ranges, held once: the setters clamp to them, the Inspector's
//! sliders span them, and the renderer holds a scene file's coordinates and detail to them.

use std::ops::RangeInclusive;

/// Metallic, roughness, alpha and alpha cutoff.
pub const FACTOR_RANGE: RangeInclusive<f32> = 0.0..=1.0;
/// HDRP's range for the albedo, normal and smoothness detail scales.
pub const DETAIL_SCALE_RANGE: RangeInclusive<f32> = 0.0..=2.0;
/// Each lane of `tiling` and `offset`, in repeats.
pub const MAP_COORD_RANGE: RangeInclusive<f32> = -1000.0..=1000.0;
/// Metres one repeat covers under a world mapping.
pub const WORLD_SIZE_RANGE: RangeInclusive<f32> = 0.01..=100.0;
/// The power the normal is raised to before it weights the triplanar projections.
pub const BLEND_SHARPNESS_RANGE: RangeInclusive<f32> = 1.0..=64.0;
/// The height map's depth, as a fraction of one repeat's width.
pub const HEIGHT_SCALE_RANGE: RangeInclusive<f32> = 0.0..=rusty_components::HEIGHT_SCALE_MAX;
