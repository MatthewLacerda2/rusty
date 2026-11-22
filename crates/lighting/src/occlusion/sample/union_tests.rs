use glam::Vec3;

use super::shaft::Polygons;
use super::sheets::{sheets, Member};
use crate::lightmap::Tri;
use crate::occlusion::gather::Face;

fn floor(nudge: f32) -> Vec<[Vec3; 3]> {
    let at = |x: usize, z: usize| Vec3::new(x as f32, 0.0, z as f32);
    let quads = (0..4).flat_map(|x| (0..4).map(move |z| (x, z)));
    let quads: Vec<[Vec3; 4]> = quads
        .map(|(x, z)| [at(x, z), at(x, z + 1), at(x + 1, z + 1), at(x + 1, z)])
        .collect();
    let off = Vec3::new(nudge, 0.0, nudge);
    let first = quads.iter().map(|q| [q[0], q[1], q[2]]);
    let second = quads.iter().map(|q| [q[0] + off, q[2] + off, q[3] + off]);
    first.chain(second).collect()
}

fn members(tris: &[[Vec3; 3]]) -> Vec<Member<'_>> {
    let face = |t| Member {
        corners: t,
        normal: Vec3::Y,
        mesh: 0,
        face: Face::Ccw,
    };
    tris.iter().map(|t| face(&t[..])).collect()
}

const EYE: (Vec3, Vec3) = (Vec3::new(1.2, 2.0, 1.2), Vec3::new(2.8, 3.0, 2.8));
const UNDER: (Vec3, Vec3) = (Vec3::new(1.2, -3.0, 1.2), Vec3::new(2.8, -2.0, 2.8));

#[test]
fn triangles_that_pair_into_no_quad_still_hide_what_is_under_them() {
    let tris = floor(0.0);
    let (fused, of) = sheets(&members(&tris));
    assert_eq!((fused.len(), of), (1, vec![0; 32]), "one plane, one sheet");
    assert!(fused[0].hides(EYE, UNDER));
    assert!(
        !fused[0].hides(UNDER, EYE),
        "its undrawn side lets it through"
    );
}

#[test]
fn a_missing_triangle_is_seen_through() {
    let mut tris = floor(0.0);
    tris.remove(2 * 4 + 2);
    assert!(!sheets(&members(&tris)).0[0].hides(EYE, UNDER));
}

#[test]
fn corners_welded_only_to_noise_fuse_nothing() {
    let tris = floor(1e-6);
    let (fused, _) = sheets(&members(&tris));
    assert!(fused.iter().all(|s| !s.hides(EYE, UNDER)));
}

#[test]
fn a_thin_faces_tip_lends_no_texel_past_a_walls_end() {
    let (e, z) = (1e-5, 1.125);
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let (q1, p, q2) = (at(0.0, z - e), at(1.0, z), at(0.0, z + e));
    let (o, r, t, u) = (at(0.0, 0.0), at(1.0, 0.0), at(1.0, 2.0), at(0.0, 2.0));
    let b = [at(2.0, 0.0), at(3.0, 0.0), at(3.0, 2.0), at(2.0, 2.0)];
    let tris = [
        [q1, p, q2],
        [o, r, p],
        [o, p, q1],
        [q2, p, t],
        [q2, t, u],
        [b[0], b[1], b[2]],
        [b[0], b[2], b[3]],
    ];
    let (fused, _) = sheets(&members(&tris));
    let eye = (at(1.125, z) + Vec3::Y, at(1.125, z) + Vec3::Y);
    let past = (at(1.125, z) - Vec3::Y, at(1.125, z) - Vec3::Y);
    assert!(fused[0].hides(eye, (at(0.5, 0.5) - Vec3::Y, at(0.5, 0.5) - Vec3::Y)));
    assert!(!fused[0].hides(eye, past), "the doorway is seen through");
}

#[test]
fn faces_join_on_their_corners_as_gathered() {
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let (p0, p1, p2, p3) = (
        at(0.137, 0.137),
        at(0.9579, 0.137),
        at(0.9579, 2.137),
        at(0.137, 2.137),
    );
    let corners = [[p0, p1, p2], [p2, p3, p0]];
    let tris = [Tri::new(p0, p1, p2, 0, 0), Tri::new(p2, p3, p0, 1, 0)];
    assert_ne!(
        tris[0].a + tris[0].e2,
        p2,
        "the rebuilt corner loses a low bit"
    );
    let polygons = Polygons::of(&tris, &corners, &[Face::Ccw; 2]);
    let mid = (p0 + p2) * 0.5;
    let (below, above) = (mid - Vec3::Y, mid + Vec3::Y);
    assert!(polygons.hides(0, (below, below), (above, above), u32::MAX));
}
