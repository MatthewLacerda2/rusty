//! The built-in primitives' lightmap UVs (#438): a Box and a Plane are lightmappable
//! out of the box (a Sphere, a Cylinder and a Capsule through the #831 unwrap), with
//! every lightmap UV inside [0, 1] and no two faces sharing one. And the Capsule's
//! shape (#1191).

use glam::{Vec2, Vec3};

use rusty_asset::lightmap_uv::interiors_overlap;

use super::primitives::{
    generate_box, generate_capsule, generate_cylinder, generate_plane, generate_sphere,
};

/// The lightmap-UV bounding rectangle of each quad (four vertices per face).
fn face_rects(vertices: &[super::Vertex]) -> Vec<([f32; 2], [f32; 2])> {
    vertices
        .chunks(4)
        .map(|quad| {
            let mut min = [f32::MAX; 2];
            let mut max = [f32::MIN; 2];
            for v in quad {
                for a in 0..2 {
                    min[a] = min[a].min(v.lightmap_uv[a]);
                    max[a] = max[a].max(v.lightmap_uv[a]);
                }
            }
            (min, max)
        })
        .collect()
}

#[test]
fn box_faces_get_disjoint_lightmap_cells() {
    let (vertices, _) = generate_box(2.0, 1.0, 3.0);
    let rects = face_rects(&vertices);
    assert_eq!(rects.len(), 6);
    for (i, (min, max)) in rects.iter().enumerate() {
        assert!(min.iter().chain(max).all(|c| (0.0..=1.0).contains(c)));
        assert!(
            max[0] > min[0] && max[1] > min[1],
            "face {i} has lightmap area"
        );
        for (other_min, other_max) in &rects[i + 1..] {
            let apart = max[0] <= other_min[0]
                || other_max[0] <= min[0]
                || max[1] <= other_min[1]
                || other_max[1] <= min[1];
            assert!(apart, "face {i}'s cell overlaps another");
        }
    }
}

#[test]
fn plane_lightmap_uv_is_its_texture_uv() {
    let (vertices, _) = generate_plane(4.0, 4.0);
    assert!(vertices.iter().all(|v| v.lightmap_uv == v.tex_coords));
}

#[test]
fn round_primitives_get_a_generated_lightmap_uv() {
    let cylinder = generate_cylinder(Vec3::ZERO, Vec3::Y, 0.5, 12).0;
    let capsule = generate_capsule(0.5, 2.0, 16, 8).0;
    for vertices in [generate_sphere(1.0, 8, 8).0, cylinder, capsule] {
        let uvs = vertices.iter().flat_map(|v| v.lightmap_uv);
        assert!(uvs.clone().all(|c| (0.0..=1.0).contains(&c)));
        assert!(uvs.into_iter().any(|c| c > 0.0), "#831 unwraps them");
    }
}

/// The Sphere's generated lightmap UV, as the primitive builds it, held to asset's
/// `assert_valid`: no two triangles share lightmap texels, and every one with surface
/// has UV area (asset's own sphere test runs a copy of this layout, since
/// `components` sits above it).
#[test]
fn the_sphere_lightmap_uv_never_overlaps() {
    let (vertices, indices) = generate_sphere(1.0, 8, 8);
    let corner = |i: u32| &vertices[i as usize];
    let tris: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let uv = |t: &[u32; 3]| t.map(|i| Vec2::from(corner(i).lightmap_uv));
    let mut surfaced = 0;
    for (i, a) in tris.iter().enumerate() {
        let [p, q, r] = a.map(|k| Vec3::from(corner(k).position));
        if (q - p).cross(r - p).length() > 1e-6 {
            let [u0, u1, u2] = uv(a);
            assert!(
                (u1 - u0).perp_dot(u2 - u0).abs() > 0.0,
                "tri {i} has UV area"
            );
            surfaced += 1;
        }
        for (j, b) in tris.iter().enumerate().skip(i + 1) {
            assert!(
                !interiors_overlap(&uv(a), &uv(b), 1e-6),
                "tris {i} and {j} overlap"
            );
        }
    }
    assert!(surfaced > 64, "the bands have surface");
}

/// Every capsule vertex lies `radius` off the inner segment, the ends reach the
/// full height, and every triangle has area and faces away from the segment.
#[test]
fn capsule_is_a_closed_outward_capsule_of_full_height() {
    let (radius, height) = (0.5, 2.0);
    let (vertices, indices) = generate_capsule(radius, height, 16, 8);
    let half_tube = height / 2.0 - radius;
    let axis_point = |p: Vec3| Vec3::Y * p.y.clamp(-half_tube, half_tube);
    let pos = |i: u32| Vec3::from(vertices[i as usize].position);
    for v in &vertices {
        let p = Vec3::from(v.position);
        assert!(((p - axis_point(p)).length() - radius).abs() < 1e-5, "{p}");
    }
    let ys = vertices.iter().map(|v| v.position[1]);
    let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
    assert!((lo + 1.0).abs() < 1e-5 && (hi - 1.0).abs() < 1e-5);
    for tri in indices.chunks(3) {
        let (a, b, c) = (pos(tri[0]), pos(tri[1]), pos(tri[2]));
        let n = (b - a).cross(c - a);
        let centre = (a + b + c) / 3.0;
        assert!(n.dot(centre - axis_point(centre)) > 0.0, "{tri:?} faces in");
    }
}
