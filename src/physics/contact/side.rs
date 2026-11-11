//! src/physics/contact/side.rs — which face of a cylinder a contact is on, and the
//! hull its side rolls as (#2071).

use std::sync::{Arc, Mutex, PoisonError};

use rapier3d::parry::shape::{ConvexPolyhedron, Cylinder, Shape};
use rapier3d::prelude::Vector;

use super::Manifold;
use crate::core::collections::Map;

/// The facets of a cylinder's side hull. Each facet a hull rolls over takes a
/// share of its energy, about 1 − ((1/2 + cos(2π/n)) / (3/2))² for a solid
/// cylinder, so at 64 a barrel keeps about 81% of its speed per turn and rolls a
/// few turns before it rests (at 32: 66%, it stops within a turn or two). A hull
/// of 128 points is still cheap for GJK.
pub(super) const SIDES: usize = 64;

/// A contact whose normal is at least this close to the cylinder's axis
/// (|cos| of the angle between them) is on a cap, which meets the world as the
/// true cylinder: within 60° of the axis (30° or more from the side), where
/// parry's own cylinder turns to its cap too.
const CAP_COS: f32 = 0.5;

/// The cylinder of `shape` when it is a rolling one: the `RoundCylinder` with no
/// rounding that `contact::cylinder` builds for a dynamic body. Any
/// other shape, an exact `Cylinder` included, is `None`.
pub(super) fn rolling(shape: &dyn Shape) -> Option<&Cylinder> {
    let round = shape.as_round_cylinder()?;
    (round.border_radius == 0.0).then_some(&round.inner_shape)
}

/// Whether the cylinder (shape 1 of the pair, or shape 2 when `second`) last
/// touched on its side: some manifold has points and none has a normal near its
/// axis. A pair with no points yet meets as the true cylinder, which stands
/// right on a cap; one that lands on its side moves to the hull a tick later.
pub(super) fn on_side(manifolds: &[Manifold], second: bool) -> bool {
    let normal = |m: &Manifold| if second { m.local_n2 } else { m.local_n1 };
    let mut touching = manifolds.iter().filter(|m| !m.points.is_empty()).peekable();
    touching.peek().is_some() && touching.all(|m| normal(m).y.abs() < CAP_COS)
}

/// The side hulls built so far, one per cylinder size. Built once and shared, so
/// a cylinder's contact costs a hull's GJK, not a hull's construction.
#[derive(Default)]
pub(super) struct Hulls {
    built: Mutex<Map<Size, Hull>>,
}

/// A cylinder's exact size, as the bits of its half height and radius.
type Size = (u32, u32);
/// A built side hull, `None` for a cylinder with no volume.
type Hull = Option<Arc<ConvexPolyhedron>>;

/// More distinct sizes than this (a cylinder scaled every tick) and the store
/// starts over, so it never grows without bound.
const KEPT: usize = 256;

impl Hulls {
    /// The hull of `cylinder`'s side, `None` for a cylinder with no volume (which
    /// keeps its own shape). Keyed by the cylinder's exact size, so the same
    /// cylinder always gets the same hull.
    pub(super) fn of(&self, cylinder: &Cylinder) -> Hull {
        let key = (cylinder.half_height.to_bits(), cylinder.radius.to_bits());
        let mut built = self.built.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(hull) = built.get(&key) {
            return hull.clone();
        }
        if built.len() >= KEPT {
            built.clear();
        }
        let hull = side_hull(cylinder).map(Arc::new);
        built.insert(key, hull.clone());
        hull
    }
}

/// A prism of [`SIDES`] facets around `cylinder`'s axis (Y), each facet tangent
/// to the cylinder: a hull lying on a facet sits at the cylinder's own radius.
fn side_hull(cylinder: &Cylinder) -> Option<ConvexPolyhedron> {
    let (r, h) = (cylinder.radius, cylinder.half_height);
    if !(r > 0.0 && h > 0.0 && r.is_finite() && h.is_finite()) {
        return None;
    }
    let step = std::f32::consts::TAU / SIDES as f32;
    let corner = r / (step / 2.0).cos();
    let points: Vec<Vector> = (0..SIDES)
        .map(|i| (i as f32 + 0.5) * step)
        .flat_map(|a| [-h, h].map(|y| Vector::new(corner * a.cos(), y, corner * a.sin())))
        .collect();
    ConvexPolyhedron::from_convex_hull(&points)
}
