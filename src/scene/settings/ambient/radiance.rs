//! The ambient's light as a function of direction (#1647): the one evaluation the
//! lightmap trace, the probe fill and (in WGSL, `lighting::ambient::ambient_radiance`) the
//! forward shader share.

use glam::Vec3;

/// The ambient radiance arriving from each direction, intensity applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmbientRadiance {
    /// `Color` mode: this colour straight up, fading linearly in `y` to a quarter of
    /// it straight down.
    Hemisphere(Vec3),
    /// `Gradient` mode (Unity's trilight): `sky` straight up, `equator` at the
    /// horizon, `ground` straight down, linear in `y` between them.
    Trilight {
        sky: Vec3,
        equator: Vec3,
        ground: Vec3,
    },
}

impl Default for AmbientRadiance {
    /// No ambient light.
    fn default() -> Self {
        Self::Hemisphere(Vec3::ZERO)
    }
}

impl From<Vec3> for AmbientRadiance {
    /// A `Color` mode ambient of radiance `sky`.
    fn from(sky: Vec3) -> Self {
        Self::Hemisphere(sky)
    }
}

impl AmbientRadiance {
    /// The radiance from (or, for a surface, the irradiance / π along normal) `dir`,
    /// a unit vector. The hemisphere's arithmetic is the lightmap trace's from before
    /// the gradient, term for term, so a `Color` scene bakes as it did.
    pub fn at(&self, dir: Vec3) -> Vec3 {
        match *self {
            Self::Hemisphere(sky) => (sky * 0.25).lerp(sky, dir.y * 0.5 + 0.5),
            Self::Trilight {
                sky,
                equator,
                ground,
            } => {
                if dir.y >= 0.0 {
                    equator.lerp(sky, dir.y)
                } else {
                    equator.lerp(ground, -dir.y)
                }
            }
        }
    }

    /// Every colour times `k`.
    pub fn scaled(&self, k: f32) -> Self {
        match *self {
            Self::Hemisphere(sky) => Self::Hemisphere(sky * k),
            Self::Trilight {
                sky,
                equator,
                ground,
            } => Self::Trilight {
                sky: sky * k,
                equator: equator * k,
                ground: ground * k,
            },
        }
    }
}
