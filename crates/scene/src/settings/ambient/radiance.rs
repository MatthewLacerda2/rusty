use glam::Vec3;

use crate::Sh9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmbientRadiance {
    Hemisphere(Vec3),
    Trilight {
        sky: Vec3,
        equator: Vec3,
        ground: Vec3,
    },
    Sky(Sh9),
}

impl Default for AmbientRadiance {
    fn default() -> Self {
        Self::Hemisphere(Vec3::ZERO)
    }
}

impl From<Vec3> for AmbientRadiance {
    fn from(sky: Vec3) -> Self {
        Self::Hemisphere(sky)
    }
}

impl AmbientRadiance {
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

    pub fn incoming(&self, dir: Vec3) -> Vec3 {
        if let Self::Sky(sh) = self {
            return sh.eval_radiance(dir);
        }
        let (sky, equator, ground) = self.poles();
        let odd = (sky - ground) * 0.5;
        let kink = (sky + ground) * 0.5 - equator;
        let y = dir.y;
        let p2 = 1.5 * y * y - 0.5;
        equator + kink * 0.5 + odd * (1.5 * y) + kink * (2.5 * p2)
    }

    #[doc(hidden)]
    pub fn at_l2(&self, n: Vec3) -> Vec3 {
        if let Self::Sky(sh) = self {
            return sh.convolved_over_pi().eval_radiance(n);
        }
        let (sky, equator, ground) = self.poles();
        let kink = (sky + ground) * 0.5 - equator;
        let p2 = 1.5 * n.y * n.y - 0.5;
        equator + kink * (0.5 + 0.625 * p2) + (sky - ground) * (0.5 * n.y)
    }

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
