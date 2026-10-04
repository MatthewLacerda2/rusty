//! src/navigation/bake/plane.rs — which triangle's plane a solid's top follows (#781).
//!
//! Every walkable triangle clipped into a cell brings its plane; when solids merge,
//! the top keeps the plane of the one that reaches highest. Two tops level within
//! [`TOP_EPS`] meet at a crease on the cell's top edge (a ramp's upper end on the
//! deck): the steeper wins, because the span's top caps it where the flatter one
//! takes over, so the crease comes out exact. A lower walkable top merged under a
//! higher one becomes the surface's `floor`, so a ramp's plane never digs under the
//! floor its foot rests on. See `navigation::surface` for how agents read it.

use glam::{Vec2, Vec3};

use super::super::Surface;

/// Two tops within this of each other count as the same top.
pub(super) const TOP_EPS: f32 = 1e-3;

/// A plane over one cell: height `y` at the cell centre, rising `slope` per metre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plane {
    pub y: f32,
    pub slope: Vec2,
}

impl Plane {
    /// The plane of triangle `t` over the cell centred at `centre` (XZ); `None` for
    /// a vertical triangle.
    pub fn of(t: [Vec3; 3], centre: Vec2) -> Option<Self> {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        if n.y.abs() <= f32::EPSILON * n.length() {
            return None;
        }
        let slope = Vec2::new(-n.x / n.y, -n.z / n.y);
        let y = t[0].y + slope.dot(centre - Vec2::new(t[0].x, t[0].z));
        Some(Self { y, slope })
    }

    /// The surface a span whose top is at `top`, with `floor` under it, stands on.
    pub fn surface(plane: Option<Self>, top: f32, floor: f32) -> Surface {
        let Some(p) = plane else {
            return Surface::FLAT;
        };
        Surface {
            centre: p.y - top,
            slope: p.slope,
            floor: top - floor,
        }
    }

    /// A total order for the bake's sort, so the result never depends on the order
    /// colliders were visited in.
    pub fn order(a: Option<Self>, b: Option<Self>) -> std::cmp::Ordering {
        let key = |p: Option<Self>| p.map(|p| [p.y, p.slope.x, p.slope.y]);
        match (key(a), key(b)) {
            (Some(a), Some(b)) => a
                .iter()
                .zip(b)
                .map(|(x, y)| x.total_cmp(&y))
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal),
            (a, b) => a.is_some().cmp(&b.is_some()),
        }
    }
}

/// The plane a merged top follows, from two tops `(max, plane)`: the higher one's,
/// or on a level pair the steeper one's (the first on a tie; a plane beats none).
/// A higher top with no plane (unwalkable, a low lip) has none: a plane under it
/// is a lower face, never its top.
pub(super) fn top_plane(a: (f32, Option<Plane>), b: (f32, Option<Plane>)) -> Option<Plane> {
    let steepness = |p: Option<Plane>| p.map_or(-1.0, |p| p.slope.length_squared());
    if b.0 > a.0 + TOP_EPS {
        b.1
    } else if a.0 > b.0 + TOP_EPS {
        a.1
    } else if steepness(b.1) > steepness(a.1) {
        b.1
    } else {
        a.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(y: f32) -> Option<Plane> {
        Some(Plane {
            y,
            slope: Vec2::ZERO,
        })
    }

    #[test]
    fn a_triangle_gives_its_plane_at_the_cell_centre() {
        let tri = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.3, 1.0),
        ];
        let p = Plane::of(tri, Vec2::new(0.5, 0.5)).expect("not vertical");
        assert!((p.y - 0.15).abs() < 1e-6);
        assert!((p.slope - Vec2::new(0.0, 0.3)).length() < 1e-6);
        let wall = [Vec3::ZERO, Vec3::X, Vec3::Y];
        assert_eq!(Plane::of(wall, Vec2::ZERO), None);
        // A big wall whose normal keeps a rounding-level y is still a wall: the
        // tolerance scales with the normal's length.
        let big = [Vec3::ZERO, Vec3::X * 100.0, Vec3::new(0.0, 100.0, 1e-6)];
        assert_eq!(Plane::of(big, Vec2::ZERO), None);
    }

    #[test]
    fn tops_exactly_top_eps_apart_are_level() {
        let up_z = Some(Plane {
            y: 0.0,
            slope: Vec2::new(0.0, 0.3),
        });
        let up_x = Some(Plane {
            y: 0.0,
            slope: Vec2::new(0.3, 0.0),
        });
        assert_eq!(top_plane((0.0, up_z), (TOP_EPS, flat(0.0))), up_z);
        assert_eq!(top_plane((TOP_EPS, flat(0.0)), (0.0, up_z)), up_z);
        assert_eq!(top_plane((0.0, up_z), (0.0, up_x)), up_z, "first on a tie");
    }

    #[test]
    fn the_higher_top_wins_and_a_level_pair_keeps_the_steeper() {
        let ramp = Some(Plane {
            y: -0.15,
            slope: Vec2::new(0.0, 0.3),
        });
        assert_eq!(top_plane((0.0, flat(0.0)), (0.2, flat(0.2))), flat(0.2));
        assert_eq!(top_plane((0.0, flat(0.0)), (0.0, ramp)), ramp);
        assert_eq!(top_plane((0.0, ramp), (0.0, flat(0.0))), ramp);
        assert_eq!(top_plane((0.0, flat(0.0)), (0.2, None)), None);
        assert_eq!(top_plane((0.0, None), (0.0, flat(0.0))), flat(0.0));
    }
}
