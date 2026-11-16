//! What a mesh-map bake can be told (#2260), after Substance Painter's *Bake Mesh
//! Maps* panel: the output size, the curvature band and each ray-traced map's ray
//! count. Every setting defaults to the value the bake had before it was a
//! setting, so a bake that names none writes the same bytes.

use super::{curvature, rays, MAX_RESOLUTION};
use rusty_procgen::image_buf::Size;

/// The most rays a texel may cast for one map: 16× the default, a quarter of its
/// grain, which keeps a 4096² bake's time bounded (rays cost time, not memory).
pub const MAX_RAYS: u32 = 1024;

/// One bake's settings. [`Settings::new`] gives the defaults at a size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// The map's extent in texels, each side `1..=`[`MAX_RESOLUTION`]. UV0's unit
    /// square stretches over it, as `Texture.Bake`'s `size` does.
    pub size: Size,
    /// How far from a crease the curvature band reaches, in the model's own units
    /// (metres for a model imported at scale). `None` is 2 % of the bounds
    /// diagonal ([`curvature::RADIUS`]). Either way the band is at least three of
    /// the mesh's average texels wide, so a low resolution still shows it.
    pub curvature_radius: Option<f32>,
    /// Rays per texel for `ao`: Substance's *Secondary Rays*, 64 by default.
    pub ao_rays: u32,
    /// Rays per texel for `thickness`, 64 by default.
    pub thickness_rays: u32,
}

impl Settings {
    /// The defaults at `size` (a bare `u32` is a square).
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            size: size.into(),
            curvature_radius: None,
            ao_rays: rays::SAMPLES,
            thickness_rays: rays::SAMPLES,
        }
    }

    /// Why these settings cannot be baked, if they can't: a side outside
    /// `1..=MAX_RESOLUTION`, a ray count outside `1..=MAX_RAYS`, a curvature radius
    /// that is not a positive finite distance.
    pub(super) fn check(&self) -> Result<(), String> {
        let Size { width, height } = self.size;
        let sides = if width == height {
            vec![("resolution", width)]
        } else {
            vec![("width", width), ("height", height)]
        };
        for (what, side) in sides {
            if !(1..=MAX_RESOLUTION).contains(&side) {
                return Err(format!("{what} {side} is outside 1..={MAX_RESOLUTION}"));
            }
        }
        for (what, n) in [
            ("ao_rays", self.ao_rays),
            ("thickness_rays", self.thickness_rays),
        ] {
            if !(1..=MAX_RAYS).contains(&n) {
                return Err(format!("{what} {n} is outside 1..={MAX_RAYS}"));
            }
        }
        match self.curvature_radius {
            Some(r) if !(r.is_finite() && r > 0.0) => {
                Err(format!("curvature_radius {r} is not a positive distance"))
            }
            _ => Ok(()),
        }
    }

    /// The band's reach for a mesh whose bounds diagonal is `diagonal`.
    pub(super) fn curvature_reach(&self, diagonal: f32) -> f32 {
        self.curvature_radius
            .unwrap_or(curvature::RADIUS * diagonal)
    }
}
