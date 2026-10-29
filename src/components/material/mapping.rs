//! Where a material's maps are read on the surface (#1573): Unity Lit's *Tiling*
//! and *Offset*, and HDRP Lit's *Base UV Mapping*, which can project the maps from
//! world space so a scaled primitive keeps one texel density on every face.

use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// Which coordinates the main maps (base colour, metallic, roughness, normal,
    /// emissive) are read at — HDRP Lit's *Base UV Mapping*, leaving out its UV1–UV3.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum UvMapping {
        /// The mesh's first UV set. The default.
        #[default]
        Uv0 = "UV0",
        /// Projected straight down the world Y axis (floors, terrain): the maps tile
        /// over world X and Z, and stretch on walls.
        Planar = "Planar",
        /// Projected along each world axis and blended by the surface normal: every
        /// face of any mesh, at any scale, shows the texture at the same size.
        Triplanar = "Triplanar",
    }
}

/// A material's map coordinates. Plain serde data, flattened into the material, so a
/// scene file names its fields beside the material's own and omits them all while
/// they hold the defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapCoordinates {
    /// Unity's *Tiling*: how many times the maps repeat across UV0 (or across
    /// `world_size` metres under a world mapping). (1, 1) by default.
    pub tiling: [f32; 2],
    /// Unity's *Offset*, added after `tiling`, in texture repeats. (0, 0) by default.
    pub offset: [f32; 2],
    /// Where the maps are read: UV0 (the default), or projected from world space.
    pub uv_mapping: UvMapping,
    /// How many metres one repeat of the texture covers under `Planar` and
    /// `Triplanar` (HDRP's *World Scale* is its inverse). 1 by default.
    pub world_size: f32,
    /// How sharply `Triplanar` turns from one projection to the next across a
    /// rounded face: the normal's components are raised to this power before they
    /// weight the three. 1 blends widely, 64 is a hard seam. 4 by default.
    pub blend_sharpness: f32,
}

impl Default for MapCoordinates {
    fn default() -> Self {
        Self {
            tiling: [1.0, 1.0],
            offset: [0.0, 0.0],
            uv_mapping: UvMapping::Uv0,
            world_size: 1.0,
            blend_sharpness: 4.0,
        }
    }
}

impl MapCoordinates {
    /// Whether these are the defaults, which a scene file leaves out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
