//! The map-coordinate fields of a material (#1573): Unity Lit's *Tiling* and
//! *Offset*, and HDRP Lit's *Base UV Mapping* with its world size and triplanar
//! blend, as rows of the same table the transparent-surface fields use
//! (`surface.rs`), so the Lua adapter, `DefineAsset`'s recipe and the Inspector take
//! them with no code of their own.
//!
//! Pure.

use std::ops::RangeInclusive;

use super::surface::{choice, number, SurfaceField, SurfaceKind, SurfaceValue};
use crate::components::{MaterialAsset, UvMapping};

/// Each lane of `tiling` and `offset`.
pub const MAP_COORD_RANGE: RangeInclusive<f32> = -1000.0..=1000.0;
/// `world_size`, in metres per repeat.
pub const WORLD_SIZE_RANGE: RangeInclusive<f32> = 0.01..=100.0;
/// `blend_sharpness`, the power the normal is raised to.
pub const BLEND_SHARPNESS_RANGE: RangeInclusive<f32> = 1.0..=64.0;

/// A pair the row's `check` clamped.
fn pair(value: SurfaceValue) -> [f32; 2] {
    match value {
        SurfaceValue::Vec2(v) => v,
        _ => [0.0; 2],
    }
}

fn world_mapped(m: &MaterialAsset) -> bool {
    m.uv_mapping != UvMapping::Uv0
}

/// Every map-coordinate field, in Inspector order.
pub static COORDINATE_FIELDS: &[SurfaceField] = &[
    SurfaceField {
        key: "uv_mapping",
        api: "UvMapping",
        label: "UV Mapping",
        kind: SurfaceKind::Choice(|| UvMapping::ALL.map(UvMapping::name).to_vec()),
        shown: |_| true,
        get: |m| SurfaceValue::Text(m.uv_mapping.name().to_owned()),
        put: |m, v| m.uv_mapping = choice(v, UvMapping::parse),
    },
    SurfaceField {
        key: "tiling",
        api: "Tiling",
        label: "Tiling",
        kind: SurfaceKind::Vec2(MAP_COORD_RANGE),
        shown: |_| true,
        get: |m| SurfaceValue::Vec2(m.tiling),
        put: |m, v| m.tiling = pair(v),
    },
    SurfaceField {
        key: "offset",
        api: "Offset",
        label: "Offset",
        kind: SurfaceKind::Vec2(MAP_COORD_RANGE),
        shown: |_| true,
        get: |m| SurfaceValue::Vec2(m.offset),
        put: |m, v| m.offset = pair(v),
    },
    SurfaceField {
        key: "world_size",
        api: "WorldSize",
        label: "World Size (m)",
        kind: SurfaceKind::Number(WORLD_SIZE_RANGE),
        shown: world_mapped,
        get: |m| SurfaceValue::Number(m.world_size),
        put: |m, v| m.world_size = number(v),
    },
    SurfaceField {
        key: "blend_sharpness",
        api: "BlendSharpness",
        label: "Blend Sharpness",
        kind: SurfaceKind::Number(BLEND_SHARPNESS_RANGE),
        shown: |m| m.uv_mapping == UvMapping::Triplanar,
        get: |m| SurfaceValue::Number(m.blend_sharpness),
        put: |m, v| m.blend_sharpness = number(v),
    },
];
