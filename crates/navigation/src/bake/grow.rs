//! A bounds change keeps every tile (keyed on the world lattice); only the cells newly covered
//! and a band along each moved edge are rebaked, as an incremental rebake does.

use super::super::{NavBounds, NavigationGraph};
use super::region::CellRect;
use super::BakeState;
use rusty_scene::Scene;

impl NavigationGraph {
    /// Whether `scene` differs from the bake in its bounds alone.
    pub(super) fn can_regrow(&self, scene: &Scene) -> bool {
        let Some(st) = self.bake_state.as_ref() else {
            return false;
        };
        let unbounded = |s: &super::super::NavMeshSettings| super::super::NavMeshSettings {
            bounds: None,
            ..s.clone()
        };
        let (b, spacing, side) = self.target_shape(scene);
        unbounded(&st.inputs.settings).bakes_like(&unbounded(&scene.nav_settings))
            && spacing == self.grid_spacing
            && side == self.tiles.side
            && b != self.bounds()
    }

    /// The cells the move dirtied, in the new frame.
    pub(super) fn regrow(&mut self, scene: &Scene, st: &mut BakeState) -> Vec<CellRect> {
        let (b, _, _) = self.target_shape(scene);
        let (old, ow, oh) = (self.tiles.origin, self.width, self.height);
        self.reframe(b);
        st.raw.reframe(b);
        let (dx, dz) = (old.0 - self.tiles.origin.0, old.1 - self.tiles.origin.1);
        st.inputs.shift(dx, dz);
        st.inputs.settings = scene.nav_settings.clone();
        let o = CellRect {
            x0: dx,
            z0: dz,
            x1: dx + ow - 1,
            z1: dz + oh - 1,
        };
        uncovered(CellRect::all(self), o)
    }

    fn reframe(&mut self, b: NavBounds) {
        let fresh = Self::empty(
            b.min_x,
            b.max_x,
            b.min_z,
            b.max_z,
            self.grid_spacing,
            self.tiles.side,
        );
        let old = self.tiles.origin;
        (self.min_x, self.max_x, self.min_z, self.max_z) = (b.min_x, b.max_x, b.min_z, b.max_z);
        (self.width, self.height) = (fresh.width, fresh.height);
        let origin = fresh.tiles.origin;
        self.tiles
            .reframe(origin, self.width, self.height, &mut self.spans);
        let (dx, dz) = (old.0 - origin.0, old.1 - origin.1);
        let mut links = std::mem::take(&mut self.offmesh.auto);
        for l in &mut links {
            for e in [&mut l.from, &mut l.to] {
                (e.gx, e.gz) = (e.gx + dx, e.gz + dz);
            }
        }
        links.retain(|l| self.in_bounds(l.from.gx, l.from.gz) && self.in_bounds(l.to.gx, l.to.gz));
        self.offmesh.auto = links;
    }
}

/// `n` outside `o`, and a band along each edge of `n` that `o` crossed: those cells lost a
/// neighbour.
fn uncovered(n: CellRect, o: CellRect) -> Vec<CellRect> {
    let Some(i) = n.intersection(o) else {
        return vec![n];
    };
    let rect = |x0, z0, x1, z1| CellRect { x0, z0, x1, z1 };
    let mut out = Vec::new();
    let mut add = |r: CellRect| out.extend(r.intersection(n));
    add(rect(n.x0, n.z0, i.x0 - 1, n.z1));
    add(rect(i.x1 + 1, n.z0, n.x1, n.z1));
    add(rect(i.x0, n.z0, i.x1, i.z0 - 1));
    add(rect(i.x0, i.z1 + 1, i.x1, n.z1));
    if o.x0 < n.x0 {
        add(rect(n.x0, n.z0, n.x0, n.z1));
    }
    if o.x1 > n.x1 {
        add(rect(n.x1, n.z0, n.x1, n.z1));
    }
    if o.z0 < n.z0 {
        add(rect(n.x0, n.z0, n.x1, n.z0));
    }
    if o.z1 > n.z1 {
        add(rect(n.x0, n.z1, n.x1, n.z1));
    }
    out
}
