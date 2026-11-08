//! src/api/physics/cast/check.rs — the argument checks the shape queries
//! share (#1881).
//!
//! A shape cast or volume query given a size it cannot build (zero, negative,
//! NaN), a non-finite position or a zero-length direction is a Lua error naming
//! the argument, instead of a NaN sweep that silently hits or misses. Unity
//! returns `false` for a zero direction; an error is louder, and the API's
//! other argument checks already raise one.

use glam::{Quat, Vec3};

use crate::components::transform::from_euler_degrees;

/// The error for `verb`'s argument `name`.
fn bad(verb: &str, name: &str, why: &str) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Physics.{verb}: {name} {why}"))
}

/// A world-space point (or vector) whose three numbers are finite.
pub(in crate::api::physics) fn point(verb: &str, name: &str, v: [f32; 3]) -> mlua::Result<Vec3> {
    let v = Vec3::from(v);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(bad(verb, name, &format!("must be finite (got {v})")))
    }
}

/// A sweep direction: finite and not zero-length.
pub(in crate::api::physics) fn direction(verb: &str, v: [f32; 3]) -> mlua::Result<Vec3> {
    let v = point(verb, "direction", v)?;
    if v.length_squared() > 0.0 {
        Ok(v)
    } else {
        Err(bad(verb, "direction", "must not be zero-length"))
    }
}

/// A radius: finite and greater than zero.
pub(in crate::api::physics) fn radius(verb: &str, r: f32) -> mlua::Result<f32> {
    if r.is_finite() && r > 0.0 {
        Ok(r)
    } else {
        Err(bad(
            verb,
            "radius",
            &format!("must be above zero (got {r})"),
        ))
    }
}

/// A box's half-extents: each finite and greater than zero.
pub(in crate::api::physics) fn half_extents(verb: &str, v: [f32; 3]) -> mlua::Result<Vec3> {
    let v = Vec3::from(v);
    if v.is_finite() && v.min_element() > 0.0 {
        Ok(v)
    } else {
        Err(bad(
            verb,
            "half_extents",
            &format!("must each be above zero (got {v})"),
        ))
    }
}

/// An optional orientation as Euler degrees (the API's rotation, as
/// `Transform.GetRotation` returns it): all three absent is no rotation, all
/// three given is that rotation, and anything between is an error.
pub(in crate::api::physics) fn orientation(
    verb: &str,
    euler: [Option<f32>; 3],
) -> mlua::Result<Quat> {
    match euler {
        [None, None, None] => Ok(Quat::IDENTITY),
        [Some(x), Some(y), Some(z)] => {
            Ok(from_euler_degrees(point(verb, "orientation", [x, y, z])?))
        }
        _ => Err(bad(verb, "orientation", "takes all three angles or none")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_must_be_positive_and_finite() {
        assert_eq!(radius("V", 0.5).unwrap(), 0.5);
        for r in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let err = radius("V", r).unwrap_err().to_string();
            assert!(err.contains("Physics.V: radius"), "{err}");
        }
        assert!(half_extents("V", [1.0, 2.0, 3.0]).is_ok());
        assert!(half_extents("V", [1.0, 0.0, 3.0]).is_err());
        assert!(half_extents("V", [1.0, f32::NAN, 3.0]).is_err());
    }

    #[test]
    fn a_direction_is_finite_and_not_zero() {
        assert_eq!(direction("V", [0.0, 0.0, 2.0]).unwrap(), Vec3::Z * 2.0);
        assert!(direction("V", [0.0; 3]).is_err());
        assert!(direction("V", [f32::NAN, 0.0, 1.0]).is_err());
        assert!(point("V", "p", [f32::INFINITY, 0.0, 0.0]).is_err());
    }

    #[test]
    fn an_orientation_is_all_or_nothing() {
        assert_eq!(orientation("V", [None; 3]).unwrap(), Quat::IDENTITY);
        let turned = orientation("V", [Some(0.0), Some(90.0), Some(0.0)]).unwrap();
        assert!((turned * Vec3::Z - Vec3::X).length() < 1e-5, "{turned}");
        assert!(orientation("V", [Some(1.0), None, None]).is_err());
        assert!(orientation("V", [Some(f32::NAN), Some(0.0), Some(0.0)]).is_err());
    }
}
