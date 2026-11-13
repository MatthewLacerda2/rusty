//! A material's detail layer (#1997): Unity HDRP Lit's *Detail Map*, a second,
//! finer (or coarser) tiling of surface variation over the main maps.
//!
//! One packed texture, HDRP's layout: R the desaturated albedo, G the normal's Y,
//! B the smoothness, A the normal's X, each neutral at 0.5. It changes the albedo,
//! the roughness and the shading normal **before lighting** (HDRP's
//! `LitDataIndividualLayer.hlsl`), so it never tints specular or reflections the way
//! the surface block `triplanar_detail` (a multiply of the lit colour) does. There is
//! no detail mask: rusty has no mask map, so the layer covers the whole surface.

use serde::{Deserialize, Serialize};

use rusty_asset::MapRef;

use super::mapping::default_tiling;
use super::surface::default_one;

/// The detail layer's fields: its map, where it is read and how strongly each of
/// its three channels applies. Absent from a material (`MaterialAsset::detail` is
/// `None`) until one of them is set.
///
/// A key it has no field for is an error naming it (`deny_unknown_fields`, as
/// `DecalBlend`), so a recipe's `tilling` is refused rather than dropped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailLayer {
    /// The packed detail map (R albedo, G normal Y, B smoothness, A normal X; 0.5
    /// neutral), read raw (its import's sRGB off). `None` draws no detail.
    #[serde(default)]
    pub map: Option<MapRef>,
    /// HDRP's detail *Tiling*: under UV0 the detail UV is the main maps' tiled UV
    /// times this (HDRP's *Link Details with Base*), so (4, 4) repeats the detail
    /// four times per repeat of the base. Under a world mapping it tiles the
    /// projection instead: (4, 4) repeats the detail four times across each
    /// `world_size` metres. (1, 1) by default.
    #[serde(default = "default_tiling")]
    pub tiling: [f32; 2],
    /// HDRP's detail *Offset*, in detail repeats, added after `tiling` (under a
    /// world mapping, after the projection). (0, 0) by default.
    #[serde(default)]
    pub offset: [f32; 2],
    /// How many metres one repeat of the detail covers under the material's
    /// `Planar` or `Triplanar` mapping, which the layer shares. 1 by default.
    #[serde(default = "default_one")]
    pub world_size: f32,
    /// HDRP's *Detail Albedo Scale* in `[0, 2]`: how far R pushes the albedo
    /// toward black or white. 1 by default.
    #[serde(default = "default_one")]
    pub albedo_scale: f32,
    /// HDRP's *Detail Normal Scale* in `[0, 2]`: how strongly A and G bend the
    /// shading normal. 1 by default.
    #[serde(default = "default_one")]
    pub normal_scale: f32,
    /// HDRP's *Detail Smoothness Scale* in `[0, 2]`: how far B pushes the
    /// smoothness (1 - roughness) toward 0 or 1. 1 by default.
    #[serde(default = "default_one")]
    pub smoothness_scale: f32,
}

impl DetailLayer {
    /// The layer with its map alone, every other field at its default; `None`
    /// without a map. What of it a material's bind group depends on.
    pub fn map_only(&self) -> Option<Self> {
        let map = self.map.clone()?;
        Some(Self {
            map: Some(map),
            ..Self::default()
        })
    }
}

impl Default for DetailLayer {
    fn default() -> Self {
        Self {
            map: None,
            tiling: default_tiling(),
            offset: [0.0; 2],
            world_size: default_one(),
            albedo_scale: default_one(),
            normal_scale: default_one(),
            smoothness_scale: default_one(),
        }
    }
}
