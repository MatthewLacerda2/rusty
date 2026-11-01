//! The ambient's light as a function of direction (#1647): the one evaluation the
//! forward shader (in WGSL, `lighting::ambient::ambient_radiance`), the lightmap
//! trace and the probe fill share.
//!
//! Two views of the same light. [`AmbientRadiance::at`] is what the forward shader
//! shades an unbaked surface with: the irradiance over π reaching a surface whose
//! normal is `n`. [`AmbientRadiance::incoming`] is the radiance arriving *along* a
//! direction, what a lightmap ray escaping the scene sees and what the probe fill
//! projects. They differ because a bake cosine-averages the radiance over a
//! hemisphere; `incoming` is built so that average is `at` again, which is what makes
//! an open baked or probe-lit surface match an unbaked one (the invariant
//! `lightmap::ambient_tests` holds for the trace, and `probe_fill`'s tests for the
//! probe fill). `Skybox` mode's sky is the third shape (#1675): an L2 SH of the
//! sky's radiance, so `incoming` reads it as is and `at` convolves it.

use glam::Vec3;

use crate::scene::Sh9;

/// The ambient light by direction, intensity applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmbientRadiance {
    /// `Color` mode: a surface facing straight up takes this colour, one facing
    /// straight down a quarter of it, linear in the normal's `y` between.
    Hemisphere(Vec3),
    /// `Gradient` mode (Unity's trilight): `sky` facing straight up, `equator`
    /// facing the horizon, `ground` facing straight down, linear in `y` between.
    Trilight {
        sky: Vec3,
        equator: Vec3,
        ground: Vec3,
    },
    /// `Skybox` mode (#1675): the sky's radiance as L2 SH, Unity's ambient probe.
    /// `at` is its cosine convolution over π, `incoming` the SH itself.
    Sky(Sh9),
}

impl Default for AmbientRadiance {
    /// No ambient light.
    fn default() -> Self {
        Self::Hemisphere(Vec3::ZERO)
    }
}

impl From<Vec3> for AmbientRadiance {
    /// A `Color` mode ambient of colour `sky`.
    fn from(sky: Vec3) -> Self {
        Self::Hemisphere(sky)
    }
}

impl AmbientRadiance {
    /// The irradiance over π that a surface with unit normal `n` takes from the
    /// ambient: what the forward shader shades an unbaked surface with, term for term.
    pub fn at(&self, n: Vec3) -> Vec3 {
        match *self {
            Self::Sky(sh) => sh.eval(n) / std::f32::consts::PI,
            Self::Hemisphere(sky) => (sky * 0.25).lerp(sky, n.y * 0.5 + 0.5),
            Self::Trilight {
                sky,
                equator,
                ground,
            } => {
                if n.y >= 0.0 {
                    equator.lerp(sky, n.y)
                } else {
                    equator.lerp(ground, -n.y)
                }
            }
        }
    }

    /// The radiance arriving along unit direction `dir`, chosen so its
    /// cosine-weighted average over the hemisphere around a normal `n` is
    /// [`Self::at`]`(n)`.
    ///
    /// Both modes are axially symmetric, `at(n) = f(n.y)`, and a cosine average
    /// scales each Legendre band `l` of a radiance by the cosine kernel's
    /// `A_l / π` (1, 2/3, 1/4 for L0–L2). So `incoming` is `f`'s bands, each divided
    /// by its factor. The hemisphere is linear in `y` (`a + b·y`, radiance
    /// `a + 1.5·b·y`), so the match is exact, and the radiance stays non-negative
    /// (a sixteenth of the colour straight down). The trilight's kink at the horizon
    /// is `|y|`, whose L0–L2 projection is `½ + ⅝·P₂(y)`: the match is exact up to
    /// that L2 cut, the accuracy of Unity's own ambient probe, and for a gradient
    /// whose equator far outshines both poles the radiance can ring below zero
    /// toward the poles, as an L2 probe does. The value is left unclamped, so the
    /// probe fill projects the exact bands; the lightmap trace clamps each escaping
    /// ray at zero (`Tracer::sky`), and `Sh9::eval` clamps what a probe gives a
    /// surface.
    ///
    /// The sky's SH is already a radiance, and its cosine average is its
    /// convolution, `at` before `at`'s clamp: the match is exact.
    pub fn incoming(&self, dir: Vec3) -> Vec3 {
        if let Self::Sky(sh) = self {
            return sh.eval_radiance(dir);
        }
        let (sky, equator, ground) = self.poles();
        // f(y) = equator + odd·y + kink·|y|.
        let odd = (sky - ground) * 0.5;
        let kink = (sky + ground) * 0.5 - equator;
        let y = dir.y;
        let p2 = 1.5 * y * y - 0.5;
        // |y| → ½ + ⅝·P₂; then L1 × 3/2 and L2 × 4.
        equator + kink * 0.5 + odd * (1.5 * y) + kink * (2.5 * p2)
    }

    /// What a bake from [`Self::incoming`] gives a surface facing `n`: [`Self::at`]
    /// up to the L2 cut (exactly `at` for the hemisphere), the reference the
    /// invariant tests hold the trace and the probe fill to.
    #[cfg(test)]
    pub fn at_l2(&self, n: Vec3) -> Vec3 {
        if let Self::Sky(sh) = self {
            return sh.convolved_over_pi().eval_radiance(n);
        }
        let (sky, equator, ground) = self.poles();
        let kink = (sky + ground) * 0.5 - equator;
        let p2 = 1.5 * n.y * n.y - 0.5;
        equator + kink * (0.5 + 0.625 * p2) + (sky - ground) * (0.5 * n.y)
    }

    /// The colour a surface facing straight up, sideways and straight down takes
    /// (the sky's SH has no poles; it never asks).
    fn poles(&self) -> (Vec3, Vec3, Vec3) {
        match *self {
            Self::Sky(_) => unreachable!("the sky's SH is read directly"),
            Self::Hemisphere(sky) => (sky, sky * 0.625, sky * 0.25),
            Self::Trilight {
                sky,
                equator,
                ground,
            } => (sky, equator, ground),
        }
    }

    /// Every colour times `k`.
    pub fn scaled(&self, k: f32) -> Self {
        match *self {
            Self::Sky(sh) => Self::Sky(sh.scaled(k)),
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
