//! How much world surface one unit of UV covers: the measure that sizes a
//! lightmap and a mesh map's texel, of a whole mesh or of one UV island.

use super::Surface;

/// World units spanned by one unit of `mesh`'s UV (the square root of world area
/// over UV area); `None` when its UVs enclose no area.
pub(crate) fn world_per_uv(mesh: &Surface) -> Option<f32> {
    let mut areas = Areas::default();
    for t in mesh.indices.chunks_exact(3) {
        areas.add(mesh, t);
    }
    areas.world_per_uv()
}

/// A running sum of triangles' world and UV areas, added in the order given: the
/// measure [`world_per_uv`] takes of a whole mesh, open to a part of one (a UV
/// island) so the part's measure is the same arithmetic.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Areas {
    world: f32,
    uv: f32,
}

impl Areas {
    /// Add the triangle `t` (its three indices into `mesh`).
    pub(crate) fn add(&mut self, mesh: &Surface, t: &[u32]) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let (pa, pb, pc) = (mesh.positions[a], mesh.positions[b], mesh.positions[c]);
        self.world += (pb - pa).cross(pc - pa).length() * 0.5;
        let (ua, ub, uc) = (mesh.uvs[a], mesh.uvs[b], mesh.uvs[c]);
        self.uv += (ub - ua).perp_dot(uc - ua).abs() * 0.5;
    }

    /// World units per unit of UV over what was added; `None` when its UVs
    /// enclose no area.
    pub(crate) fn world_per_uv(self) -> Option<f32> {
        (self.uv >= 1e-8 && self.world > 0.0).then(|| (self.world / self.uv).sqrt())
    }
}
