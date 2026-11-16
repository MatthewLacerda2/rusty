//! What a mesh-map bake can be told (#2260), after Substance Painter's *Bake Mesh
//! Maps* panel: the output size, the curvature band and each ray-traced map's ray
//! count. Every setting defaults to the value the bake had before it was a
//! setting, so a bake that names none writes the same bytes.

use super::{curvature, rays, MAX_RESOLUTION};
use rusty_procgen::image_buf::Size;

/// The most rays a texel may cast for one map: 16× the default, a quarter of its
/// grain, which keeps a bake's time bounded (rays cost time, not memory).
///
/// Measured at the cap (4 threads in a cloud container, a release build, the L
/// fixture's floor and wall at 256², 16 074 covered texels): about 95 µs per
/// covered texel for `ao`, the same for `thickness`, and about 1.3× that in a dev
/// build. Scaled by texel count, not run: a fully covered 4096² map is about 26 min
/// per ray-traced map, 8192² about 1.8 h. Time goes with the ray count, so the
/// default 64 is a sixteenth of that.
pub const MAX_RAYS: u32 = 1024;

/// One bake's settings. [`Settings::new`] gives the defaults at a size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// The map's extent in texels, each side `1..=`[`MAX_RESOLUTION`]. UV0's unit
    /// square stretches over it, as `Texture.Bake`'s `size` does.
    pub size: Size,
    /// How far from a crease the curvature band reaches, in the model's own units
    /// (metres for a model imported at scale). `None` is 2 % of the bounds
    /// diagonal (`curvature::RADIUS`). Either way the band is at least three of
    /// the mesh's average texels wide, so a low resolution still shows it, and an
    /// explicit radius is at most the bounds diagonal (`Settings::check_reach`).
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
    /// that is not a positive finite distance. Its bound by the model's size is
    /// [`Settings::check_reach`], once the bounds are known.
    pub(super) fn check(&self) -> Result<(), String> {
        let Size { width, height } = self.size;
        let side = 1..=MAX_RESOLUTION;
        if !(side.contains(&width) && side.contains(&height)) {
            return Err(if width == height {
                format!("resolution {width} is outside 1..={MAX_RESOLUTION}")
            } else {
                format!("size {{{width}, {height}}} has a side outside 1..={MAX_RESOLUTION}")
            });
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

    /// Why `curvature_radius` is too wide for a mesh whose bounds diagonal is
    /// `diagonal`, if it is: a band wider than the whole model reads every crease
    /// from every texel, so a radius typed in millimetres' place of metres would
    /// run for hours to bake a map that is all band.
    pub(super) fn check_reach(&self, diagonal: f32) -> Result<(), String> {
        match self.curvature_radius {
            Some(r) if r > diagonal => Err(format!(
                "curvature_radius {r} is above the model's bounds diagonal {diagonal} \
                 (both in the model's units, metres at import scale)"
            )),
            _ => Ok(()),
        }
    }

    /// The band's reach for a mesh whose bounds diagonal is `diagonal`.
    pub(super) fn curvature_reach(&self, diagonal: f32) -> f32 {
        self.curvature_radius
            .unwrap_or(curvature::RADIUS * diagonal)
    }
}
