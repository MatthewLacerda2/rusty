//! src/components/material/decal.rs — what a material changes when a decal stamps
//! it (#638). A decal points at a library material, Unity's way: the material's
//! albedo, normal, metallic and roughness (factors and maps) are the decal's surface
//! properties, and this block says how strongly each one replaces the surface's own.

use serde::{Deserialize, Serialize};

fn one() -> f32 {
    1.0
}

fn default_angle_fade() -> f32 {
    60.0
}

/// Per-channel blend weights for a decal material, each in `[0, 1]` and scaled by
/// the decal's coverage (texture alpha × tint alpha × fades). A weight of `0`
/// leaves that channel of the receiving surface alone, so a decal that changes only
/// `roughness` is a wet patch, and one that changes only `normal` is a dent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecalBlend {
    /// How much the decal's albedo (`base_color` × `base_color_map`) replaces the
    /// surface's.
    #[serde(default = "one")]
    pub albedo: f32,
    /// How much the decal's `normal_map` bends the surface normal. No map, no bend.
    #[serde(default = "one")]
    pub normal: f32,
    /// How much the decal's `metallic` (× `metallic_map`) replaces the surface's.
    #[serde(default = "one")]
    pub metallic: f32,
    /// How much the decal's `roughness` (× `roughness_map`) replaces the surface's.
    #[serde(default = "one")]
    pub roughness: f32,
    /// The ambient occlusion the decal adds where it covers, in `[0, 1]`: `1` adds
    /// none, `0.5` halves the indirect light (a crater's inside).
    #[serde(default = "one")]
    pub occlusion: f32,
    /// Degrees in `[0, 90]`: the decal is whole on a surface turned up to this far
    /// from facing the projector, and fades out over the next 15°, so a surface
    /// running along the projection axis is never streaked.
    #[serde(default = "default_angle_fade")]
    pub angle_fade: f32,
}

impl Default for DecalBlend {
    fn default() -> Self {
        Self {
            albedo: 1.0,
            normal: 1.0,
            metallic: 1.0,
            roughness: 1.0,
            occlusion: 1.0,
            angle_fade: default_angle_fade(),
        }
    }
}

impl DecalBlend {
    /// The blend of a decal with no material: it changes the albedo alone, as the
    /// pre-#638 decals did, but lit.
    pub const ALBEDO_ONLY: Self = Self {
        albedo: 1.0,
        normal: 0.0,
        metallic: 0.0,
        roughness: 0.0,
        occlusion: 1.0,
        angle_fade: 60.0,
    };

    /// Whether every field holds its default (so the scene file can omit it).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The weights clamped to `[0, 1]` and the angle to `[0, 90]` degrees.
    pub fn clamped(self) -> Self {
        let unit = |v: f32| v.clamp(0.0, 1.0);
        Self {
            albedo: unit(self.albedo),
            normal: unit(self.normal),
            metallic: unit(self.metallic),
            roughness: unit(self.roughness),
            occlusion: unit(self.occlusion),
            angle_fade: self.angle_fade.clamp(0.0, 90.0),
        }
    }
}
