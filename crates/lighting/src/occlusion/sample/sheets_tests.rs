//! Occluder fusion on a plane (#1714, part 3): tiles that hide together what none
//! hides alone, and never a gap between them.

use glam::Vec3;

use super::sheets::{sheets, Member};
use crate::occlusion::gather::Face;

/// A floor tile over `[x0, x1] × [z0, z1]` at y = 0, its drawn side up, wound
/// counter-clockwise seen from above.
fn tile(x0: f32, x1: f32, z0: f32, z1: f32) -> [Vec3; 4] {
    tile_at(0.0, (x0, x1), (z0, z1))
}

/// The same tile at height `y`.
fn tile_at(y: f32, (x0, x1): (f32, f32), (z0, z1): (f32, f32)) -> [Vec3; 4] {
    [
        Vec3::new(x0, y, z0),
        Vec3::new(x0, y, z1),
        Vec3::new(x1, y, z1),
        Vec3::new(x1, y, z0),
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
    // A 1.5 mm gap off the texel grid: the texels it crosses are not set, so it
    // is seen through.
    let tiles = [tile(-2.0, 1.9985, -2.0, 6.0), tile(2.0, 6.0, -2.0, 6.0)];
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
    // Two small tiles on one plane far apart fuse nothing: the sheet would be
    // mostly empty, so each is its own polygon.
    let apart = [tile(-2.0, -1.0, -2.0, -1.0), tile(20.0, 21.0, 20.0, 21.0)];
    let (none, of) = sheets(&floor(&apart));
    assert!(none.is_empty() && of == vec![u32::MAX; 2]);
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

#[test]
fn a_slit_a_fifth_of_a_millimetre_wide_is_seen_through() {
    let tiles = [tile(-2.0, 1.9999, -2.0, 6.0), tile(2.0001, 6.0, -2.0, 6.0)];
    assert!(!sheets(&floor(&tiles)).0[0].hides(EYE, UNDER));
}

#[test]
fn faces_a_millimetre_apart_make_no_one_sheet() {
    // Both round to one plane, but a segment slips through the step between
    // them: it crosses y = 0.0014 at x ≈ 51.8, past the first tile, and
    // y = 0.0006 at x ≈ 48.2, short of the second.
    let tiles = [
        tile_at(0.0014, (0.0, 50.0), (-1.0, 1.0)),
        tile_at(0.0006, (50.0, 100.0), (-1.0, 1.0)),
    ];
    let (from, to) = (Vec3::new(100.0, 0.012, 0.0), Vec3::new(0.0, -0.01, 0.0));
    let (fused, of) = sheets(&floor(&tiles));
    assert!(fused.iter().all(|s| !s.hides((from, from), (to, to))));
    assert_eq!(of, vec![u32::MAX; 2], "each is left to its polygon");
}
