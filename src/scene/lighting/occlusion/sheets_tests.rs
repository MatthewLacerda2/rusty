//! Occluder fusion on a plane (#1714, part 3): tiles that hide together what none
//! hides alone, and never a gap between them.

use glam::Vec3;

use super::gather::Face;
use super::sheets::{sheets, Member};

/// A floor tile over `[x0, x1] × [z0, z1]` at y = 0, its drawn side up, wound
/// counter-clockwise seen from above.
fn tile(x0: f32, x1: f32, z0: f32, z1: f32) -> [Vec3; 4] {
    [
        Vec3::new(x0, 0.0, z0),
        Vec3::new(x0, 0.0, z1),
        Vec3::new(x1, 0.0, z1),
        Vec3::new(x1, 0.0, z0),
    ]
}

fn floor(tiles: &[[Vec3; 4]]) -> Vec<Member<'_>> {
    let mut members = Vec::new();
    for (i, t) in tiles.iter().enumerate() {
        members.push(Member {
            corners: t,
            normal: Vec3::Y,
            mesh: i as u32,
            face: Face::Ccw,
        });
    }
    members
}

/// An eye above the floor, and a box under it astride the seam at x = 2.
const EYE: (Vec3, Vec3) = (Vec3::new(1.5, 2.0, 1.5), Vec3::new(2.5, 3.0, 2.5));
const UNDER: (Vec3, Vec3) = (Vec3::new(1.5, -3.0, 1.5), Vec3::new(2.5, -2.0, 2.5));

#[test]
fn two_tiles_hide_what_straddles_their_seam_and_a_gap_does_not() {
    let tiles = [tile(-2.0, 2.0, -2.0, 6.0), tile(2.0, 6.0, -2.0, 6.0)];
    let (fused, of) = sheets(&floor(&tiles));
    assert_eq!((fused.len(), of), (1, vec![0, 0]), "one plane, one sheet");
    assert!(fused[0].hides(EYE, UNDER));
    // From below, the tiles' undrawn side lets the view through.
    assert!(!fused[0].hides(UNDER, EYE));
    // A 5 cm gap off the texel grid is seen through.
    let tiles = [tile(-2.0, 1.95, -2.0, 6.0), tile(2.0, 6.0, -2.0, 6.0)];
    assert!(!sheets(&floor(&tiles)).0[0].hides(EYE, UNDER));
}

#[test]
fn a_box_reaching_the_plane_or_past_its_edge_is_not_hidden() {
    let tiles = [tile(-2.0, 2.0, -2.0, 6.0), tile(2.0, 6.0, -2.0, 6.0)];
    let (fused, _) = sheets(&floor(&tiles));
    let touching = (UNDER.0, Vec3::new(2.5, 0.0, 2.5));
    assert!(!fused[0].hides(EYE, touching));
    let past = (Vec3::new(5.5, -3.0, 1.5), Vec3::new(9.0, -2.0, 2.5));
    assert!(!fused[0].hides(EYE, past));
}

#[test]
fn a_member_and_a_lone_face_are_left_to_their_polygons() {
    let tiles = [tile(-2.0, 2.0, -2.0, 6.0), tile(2.0, 6.0, -2.0, 6.0)];
    let (fused, _) = sheets(&floor(&tiles));
    assert!(fused[0].holds(1) && !fused[0].holds(2));
    let (alone, of) = sheets(&floor(&tiles[..1]));
    assert!(alone.is_empty() && of == vec![u32::MAX]);
}

#[test]
fn a_plane_far_out_or_huge_bakes_without_a_panic() {
    let far = 1.0e30;
    let tiles = [tile(-far, 0.0, -far, far), tile(0.0, far, -far, far)];
    let (fused, _) = sheets(&floor(&tiles));
    // Coarse texels, but whatever it says is still one way: never past the edge.
    let past = (
        Vec3::new(2.0 * far, -3.0, 0.0),
        Vec3::new(3.0 * far, -2.0, 1.0),
    );
    assert!(!fused[0].hides(EYE, past));
}
