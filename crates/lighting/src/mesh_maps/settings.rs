use super::{curvature, rays, MAX_RESOLUTION};
use rusty_procgen::image_buf::Size;

/// Per texel per map: 16× the default, bounding a bake's time (rays cost time, not memory).
pub const MAX_RAYS: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub size: Size,
    pub curvature_radius: Option<f32>,
    pub ao_rays: u32,
    pub thickness_rays: u32,
}

impl Settings {
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            size: size.into(),
            curvature_radius: None,
            ao_rays: rays::SAMPLES,
            thickness_rays: rays::SAMPLES,
        }
    }

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

    pub(super) fn check_reach(&self, diagonal: f32) -> Result<(), String> {
        match self.curvature_radius {
            Some(r) if r > diagonal => Err(format!(
                "curvature_radius {r} is above the model's bounds diagonal {diagonal} \
                 (both in the model's units, metres at import scale)"
            )),
            _ => Ok(()),
        }
    }

    pub(super) fn curvature_reach(&self, diagonal: f32) -> f32 {
        self.curvature_radius
            .unwrap_or(curvature::RADIUS * diagonal)
    }
}
