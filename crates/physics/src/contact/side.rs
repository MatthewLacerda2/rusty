use std::sync::{Arc, Mutex, PoisonError};

use rapier3d::parry::shape::{ConvexPolyhedron, Cylinder, Shape};
use rapier3d::prelude::Vector;

use super::Manifold;
use rusty_core::collections::Map;

/// Each facet a hull rolls over takes energy: a 64-facet barrel keeps ~81% of its speed per turn, 32 keeps 66%.
pub(super) const SIDES: usize = 64;

/// A contact whose normal is within 60° of the cylinder's axis (|cos| at least this) is on a cap,
/// which meets the world as the true cylinder, as parry's own cylinder does.
const CAP_COS: f32 = 0.5;

pub(super) fn rolling(shape: &dyn Shape) -> Option<&Cylinder> {
    let round = shape.as_round_cylinder()?;
    (round.border_radius == 0.0).then_some(&round.inner_shape)
}

/// A pair with no points yet meets as the true cylinder, which stands right on a cap;
/// one that lands on its side moves to the hull a tick later.
pub(super) fn on_side(manifolds: &[Manifold], second: bool) -> bool {
    let normal = |m: &Manifold| if second { m.local_n2 } else { m.local_n1 };
    let mut touching = manifolds.iter().filter(|m| !m.points.is_empty()).peekable();
    touching.peek().is_some() && touching.all(|m| normal(m).y.abs() < CAP_COS)
}

#[derive(Default)]
pub(super) struct Hulls {
    built: Mutex<Map<Size, Hull>>,
}

type Size = (u32, u32);
type Hull = Option<Arc<ConvexPolyhedron>>;

const KEPT: usize = 256;

impl Hulls {
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

/// Each facet is tangent to the cylinder: a hull lying on a facet sits at its radius.
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
