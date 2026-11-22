use std::ops::RangeInclusive;

pub const FACTOR_RANGE: RangeInclusive<f32> = 0.0..=1.0;
pub const DETAIL_SCALE_RANGE: RangeInclusive<f32> = 0.0..=2.0;
pub const MAP_COORD_RANGE: RangeInclusive<f32> = -1000.0..=1000.0;
pub const WORLD_SIZE_RANGE: RangeInclusive<f32> = 0.01..=100.0;
pub const BLEND_SHARPNESS_RANGE: RangeInclusive<f32> = 1.0..=64.0;
pub const HEIGHT_SCALE_RANGE: RangeInclusive<f32> = 0.0..=rusty_components::HEIGHT_SCALE_MAX;
