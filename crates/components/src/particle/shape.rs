use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use rusty_core::random::Random;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum EmitShape {
    #[default]
    Point,
    /// Radius in world units.
    Sphere { radius: f32 },
    /// Radius in world units.
    Hemisphere { radius: f32 },
    /// Full extents in world units.
    Box { size: Vec3 },
    /// Half-angle in degrees.
    Cone { angle: f32, radius: f32 },
    /// Radius in world units.
    Circle { radius: f32 },
}

/// Emit from the whole shape, or only from its surface / edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmitFrom {
    #[default]
    Volume,
    Surface,
}

impl EmitShape {
    pub const NAMES: [&'static str; 6] = ["Point", "Sphere", "Hemisphere", "Box", "Cone", "Circle"];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Sphere { .. } => "Sphere",
            Self::Hemisphere { .. } => "Hemisphere",
            Self::Box { .. } => "Box",
            Self::Cone { .. } => "Cone",
            Self::Circle { .. } => "Circle",
        }
    }

    pub fn from_parts(name: &str, radius: f32, angle: f32, size: Vec3) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "point" => Self::Point,
            "sphere" => Self::Sphere { radius },
            "hemisphere" => Self::Hemisphere { radius },
            "box" => Self::Box { size },
            "cone" => Self::Cone { angle, radius },
            "circle" => Self::Circle { radius },
            _ => return None,
        })
    }

    /// Sample one spawn: `(offset from the emitter, unit launch direction)`.
    /// `axis` must be normalised (or zero, which launches nothing).
    pub fn sample(&self, from: EmitFrom, axis: Vec3, rng: &mut Random) -> (Vec3, Vec3) {
        let surface = from == EmitFrom::Surface;
        match *self {
            Self::Point => (Vec3::ZERO, axis),
            Self::Sphere { radius } => radial(ball(surface, rng) * radius, axis),
            Self::Hemisphere { radius } => {
                let p = ball(surface, rng);
                let p = if p.dot(axis) < 0.0 {
                    p - 2.0 * p.dot(axis) * axis
                } else {
                    p
                };
                radial(p * radius, axis)
            }
            Self::Box { size } => (box_point(size * 0.5, surface, rng), axis),
            Self::Cone { angle, radius } => cone(angle, radius, surface, axis, rng),
            Self::Circle { radius } => {
                let (u, v) = basis(axis);
                let d = disc(surface, rng);
                let offset = (u * d.x + v * d.y) * radius;
                radial(offset, offset.normalize_or_zero())
            }
        }
    }
}

/// A point in (or, for `surface`, on) the unit ball.
fn ball(surface: bool, rng: &mut Random) -> Vec3 {
    if surface {
        rng.on_unit_sphere()
    } else {
        rng.inside_unit_sphere()
    }
}

/// A point in (or on the edge of) the unit disc.
fn disc(surface: bool, rng: &mut Random) -> Vec2 {
    if surface {
        let phi = rng.value() as f32 * std::f32::consts::TAU;
        Vec2::new(phi.cos(), phi.sin())
    } else {
        rng.inside_unit_circle()
    }
}

fn radial(offset: Vec3, fallback: Vec3) -> (Vec3, Vec3) {
    let dir = offset.normalize_or_zero();
    (offset, if dir == Vec3::ZERO { fallback } else { dir })
}

/// Two unit vectors perpendicular to `axis` (and each other).
fn basis(axis: Vec3) -> (Vec3, Vec3) {
    if axis == Vec3::ZERO {
        (Vec3::X, Vec3::Z)
    } else {
        axis.any_orthonormal_pair()
    }
}

fn box_point(half: Vec3, surface: bool, rng: &mut Random) -> Vec3 {
    let mut p = Vec3::new(signed(rng), signed(rng), signed(rng));
    if surface {
        let face = rng.range_i64(0, 3) as usize;
        p[face] = p[face].signum();
    }
    p * half
}

fn cone(angle_deg: f32, radius: f32, surface: bool, axis: Vec3, rng: &mut Random) -> (Vec3, Vec3) {
    let (u, v) = basis(axis);
    let angle = angle_deg.to_radians();
    let d = disc(surface, rng);
    let offset = (u * d.x + v * d.y) * radius;
    let (theta, phi) = if surface {
        (angle, d.y.atan2(d.x))
    } else {
        let cos = 1.0 - rng.value() as f32 * (1.0 - angle.cos());
        (
            cos.clamp(-1.0, 1.0).acos(),
            rng.value() as f32 * std::f32::consts::TAU,
        )
    };
    let dir = axis * theta.cos() + (u * phi.cos() + v * phi.sin()) * theta.sin();
    (offset, dir)
}

pub(super) fn signed(rng: &mut Random) -> f32 {
    rng.value() as f32 * 2.0 - 1.0
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
