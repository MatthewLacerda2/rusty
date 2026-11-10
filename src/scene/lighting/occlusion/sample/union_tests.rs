//! A sheet's faces hold together what none holds alone (#1981): a floor of
//! triangles that pair into no quad still hides what is under it, a missing
//! triangle is seen through, and corners welded only to noise fuse nothing.

use glam::Vec3;

use super::sheets::{sheets, Member};
use crate::scene::lighting::occlusion::gather::Face;

/// A 4 × 4 m floor at y = 0 of 1 m quads, each as two triangles drawn up, every
/// quad's first triangle before every quad's second; `nudge` moves the second
/// triangles' corners by that much, so no edge they share is the same bits.
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

/// An eye above the floor, and a box under it across many triangles' edges.
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
    // Quad (2, 2)'s first triangle, under the middle of the box.
    tris.remove(2 * 4 + 2);
    assert!(!sheets(&members(&tris)).0[0].hides(EYE, UNDER));
}

#[test]
fn corners_welded_only_to_noise_fuse_nothing() {
    // Each diagonal is two edges a micrometre apart: the slit between them may
    // be real, so the texels it crosses are not held.
    let tris = floor(1e-6);
    let (fused, _) = sheets(&members(&tris));
    assert!(fused.iter().all(|s| !s.hides(EYE, UNDER)));
}
