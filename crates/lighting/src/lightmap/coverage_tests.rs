//! Conservative coverage (#1687): a chart thinner than a texel still owns texels,
//! each traced from a point on its own surface, and a texel whose centre a triangle
//! covers is traced at that centre exactly as before.

use glam::{Vec2, Vec3};

use super::raster::{rasterize, world_per_uv, INSET};
use super::tests::settings;
use super::*;

/// The lightmap edge the strips are rasterized at.
const S: u32 = 16;

/// `STRIPS` flat 10 × 1 m strips side by side at y = 0, facing up, each one chart
/// 0.3 texels tall in a 16² lightmap, between two rows of texel centres: strip `k`
/// spans v ∈ [2k + 0.6, 2k + 0.9] texels, so no texel centre lies in any of them.
const STRIPS: u32 = 8;

fn strips() -> BakeMesh {
    let mut mesh = BakeMesh {
        entity: 1,
        ..Default::default()
    };
    for k in 0..STRIPS {
        let (z, v) = (k as f32, (2 * k) as f32);
        let base = mesh.positions.len() as u32;
        let p = [(0.0, z), (10.0, z), (10.0, z + 1.0), (0.0, z + 1.0)];
        mesh.positions.extend(p.map(|(x, z)| Vec3::new(x, 0.0, z)));
        let uv = [(0.8, 0.6), (15.2, 0.6), (15.2, 0.9), (0.8, 0.9)];
        let uv = uv.map(|(u, dv)| Vec2::new(u, v + dv) / S as f32);
        mesh.lightmap_uvs.extend(uv);
        mesh.indices.extend([0, 2, 1, 0, 3, 2].map(|i| base + i));
    }
    mesh.normals = vec![Vec3::Y; mesh.positions.len()];
    mesh.runs = vec![BakeRun {
        end: mesh.indices.len() as u32,
        albedo: Vec3::splat(0.5),
        emissive: Vec3::ZERO,
        occludes: true,
    }];
    mesh
}

#[test]
fn every_sub_texel_strip_owns_its_row_traced_on_its_own_surface() {
    let points = rasterize(&strips(), S);
    for k in 0..STRIPS {
        let row: Vec<_> = points.iter().filter(|p| p.index / S == 2 * k).collect();
        assert_eq!(row.len(), S as usize, "strip {k} owns its whole row");
        for p in row {
            let (x, z) = (p.position.x, p.position.z - k as f32);
            let on = (0.0..=10.0).contains(&x) && (0.0..=1.0).contains(&z);
            assert!(on && p.position.y == 0.0, "strip {k}: {}", p.position);
        }
    }
    assert_eq!(
        points.len(),
        (S * STRIPS) as usize,
        "nothing past the strips"
    );
}

#[test]
fn sub_texel_strips_bake_lit_not_black() {
    let scene = BakeScene {
        shadowmask: Default::default(),
        meshes: vec![strips()],
        lights: Vec::new(),
        sky: Vec3::ONE.into(),
        diffuse: Default::default(),
    };
    // A density that sizes the strips' map at exactly S, under the page's cap.
    let settings = BakeSettings {
        texels_per_unit: (S as f32 - 0.5) / world_per_uv(&strips().surface()).unwrap(),
        ..settings()
    };
    let map = &bake(&scene, &settings)[0];
    assert_eq!(map.size, S, "the density holds the strips under a texel");
    for k in 0..STRIPS as usize {
        let row = &map.texels[2 * k * S as usize..][..S as usize];
        assert!(row.iter().all(|t| t.x > 0.5), "strip {k} is dark: {row:?}");
    }
}

/// One triangle over UV (0, 0), (1, 0), (0, 1), laid out at y = `y` in world units
/// of one texel, so a texel's point reads straight off its world position.
fn corner(y: f32, scale: f32) -> BakeMesh {
    let uv = [Vec2::ZERO, Vec2::X, Vec2::Y].map(|u| u * scale);
    BakeMesh {
        positions: uv
            .map(|u| Vec3::new(u.x * S as f32, y, u.y * S as f32))
            .to_vec(),
        normals: vec![Vec3::Y; 3],
        lightmap_uvs: uv.to_vec(),
        indices: vec![0, 2, 1],
        ..Default::default()
    }
}

#[test]
fn centre_texels_keep_their_centre_and_the_edge_ring_is_traced_inset_from_every_edge() {
    // The hypotenuse u + v = S - 0.5 crosses the squares of the texels x + y = S - 1
    // but not their centres: each is traced in its texel, INSET off every edge.
    let edge = S as f32 - 0.5;
    let points = rasterize(&corner(0.0, edge / S as f32), S);
    assert_eq!(points.len(), (S * (S + 1) / 2) as usize, "x + y < S");
    for p in &points {
        let (x, y) = ((p.index % S) as f32, (p.index / S) as f32);
        let (u, v) = (p.position.x, p.position.z);
        let ok = match x + y + 1.0 < S as f32 {
            true => Vec2::new(u - x - 0.5, v - y - 0.5).length() < 1e-4,
            false => {
                [u, v, (edge - u - v) / 2f32.sqrt()]
                    .iter()
                    .all(|&d| d >= INSET - 1e-4)
                    && (x..=x + 1.0).contains(&u)
                    && (y..=y + 1.0).contains(&v)
            }
        };
        assert!(ok, "texel ({x}, {y}) traced at {u}, {v}");
    }
}

/// `a` with `b`'s one triangle after its own.
fn join(mut a: BakeMesh, b: BakeMesh) -> BakeMesh {
    a.positions.extend(b.positions);
    a.normals.extend(b.normals);
    a.lightmap_uvs.extend(b.lightmap_uvs);
    a.indices.extend([3, 5, 4]);
    a
}

#[test]
fn a_shared_texel_goes_to_the_first_triangle_and_a_centre_beats_an_overlap() {
    // Two sub-texel triangles in texel (0, 0), neither over its centre: the first wins.
    let tiny = |y| corner(y, 0.3 / S as f32);
    let points = rasterize(&join(tiny(0.0), tiny(1.0)), S);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].position.y, 0.0, "the first triangle keeps it");
    // A later triangle over the centre takes it from an earlier one that only overlaps.
    let later = rasterize(&join(tiny(0.0), corner(1.0, 1.0)), S);
    let first = later.into_iter().find(|p| p.index == 0);
    assert_eq!(first.map(|p| p.position.y), Some(1.0), "the centre wins");
}
