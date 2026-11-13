//! A town's meshes made as an imported level's are (#1981): one mesh of many
//! boxes, each face tessellated at most `STEP` apart with a quad's two
//! triangles never next to each other (so the bake's `Polygon::merge` pairs
//! none, as with a mesh an exporter reordered), and bumpy ground.

use glam::Vec3;

use crate::authoring::{create_drawn_entity, Primitive};
use crate::Scene;
use rusty_components::mesh::{MeshRevision, Vertex};

/// The tessellation's spacing, metres.
const STEP: f32 = 1.0;

/// A static mesh of `boxes` (world space), tessellated.
pub fn mesh(scene: &mut Scene, name: &str, boxes: &[(Vec3, Vec3)]) -> u32 {
    let mut quads = Vec::new();
    for &(lo, hi) in boxes {
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                quads.extend(face(lo, hi, axis, sign));
            }
        }
    }
    entity(scene, name, &quads)
}

/// The ground over the slab `(lo, hi)`: its top, a bumpy grid of `STEP`.
pub fn ground(scene: &mut Scene, lo: Vec3, hi: Vec3) -> u32 {
    let n = ((hi.x - lo.x) / STEP).ceil() as usize;
    let m = ((hi.z - lo.z) / STEP).ceil() as usize;
    let at = |i: usize, j: usize| {
        let (x, z) = (
            lo.x + (hi.x - lo.x) * i as f32 / n as f32,
            lo.z + (hi.z - lo.z) * j as f32 / m as f32,
        );
        Vec3::new(x, hi.y + 0.05 * (x * 1.3).sin() * (z * 0.9).cos(), z)
    };
    let quads: Vec<[Vec3; 4]> = (0..n)
        .flat_map(|i| (0..m).map(move |j| (i, j)))
        .map(|(i, j)| [at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)])
        .collect();
    entity(scene, "Ground", &quads)
}

/// One side of the box `(lo, hi)`, normal along `axis` by `sign`, as quads of
/// at most `STEP` wound outward.
fn face(lo: Vec3, hi: Vec3, axis: usize, sign: f32) -> Vec<[Vec3; 4]> {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let (u, v) = if sign > 0.0 { (u, v) } else { (v, u) };
    let (nu, nv) = (cuts(lo[u], hi[u]), cuts(lo[v], hi[v]));
    let point = |a: f32, b: f32| {
        let mut p = Vec3::ZERO;
        p[axis] = if sign > 0.0 { hi[axis] } else { lo[axis] };
        (p[u], p[v]) = (lo[u] + (hi[u] - lo[u]) * a, lo[v] + (hi[v] - lo[v]) * b);
        p
    };
    let mut quads = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let (a0, a1) = (i as f32 / nu as f32, (i + 1) as f32 / nu as f32);
            let (b0, b1) = (j as f32 / nv as f32, (j + 1) as f32 / nv as f32);
            quads.push([point(a0, b0), point(a1, b0), point(a1, b1), point(a0, b1)]);
        }
    }
    quads
}

fn cuts(lo: f32, hi: f32) -> usize {
    ((hi - lo) / STEP).ceil().max(1.0) as usize
}

/// A static entity at the origin whose mesh is `quads`: every quad's first
/// triangle, then every quad's second.
pub fn entity(scene: &mut Scene, name: &str, quads: &[[Vec3; 4]]) -> u32 {
    let mut vertices = Vec::with_capacity(quads.len() * 4);
    for q in quads {
        let n = (q[1] - q[0]).cross(q[2] - q[0]).normalize_or_zero();
        vertices.extend(q.iter().map(|&p| Vertex::new(p, n, [0.0, 0.0])));
    }
    let first = (0..quads.len() as u32).flat_map(|k| [4 * k, 4 * k + 1, 4 * k + 2]);
    let second = (0..quads.len() as u32).flat_map(|k| [4 * k, 4 * k + 2, 4 * k + 3]);
    let indices = first.chain(second).collect();
    let id = create_drawn_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).expect("a new entity");
    (t.position, t.scale) = (Vec3::ZERO, Vec3::ONE);
    drop(t);
    scene.world.set_static(id, true);
    let mut mesh = scene.world.mesh_mut(id).expect("a box");
    (mesh.vertices, mesh.indices) = (vertices, indices);
    mesh.revision = MeshRevision::next();
    id
}
