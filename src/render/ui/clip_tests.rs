use glam::{Vec2, Vec4};

use super::*;

fn soft(lo: [f32; 2], hi: [f32; 2], feather: f32) -> SoftRect {
    SoftRect {
        lo: Vec2::from_array(lo),
        hi: Vec2::from_array(hi),
        feather: Vec4::splat(feather),
    }
}

#[test]
fn an_intersection_keeps_each_edges_own_feather() {
    // A hard panel 0..100 around a soft list 10..200 that overflows it right/top.
    let panel = soft([0.0, 0.0], [100.0, 100.0], 0.0);
    let list = soft([10.0, 10.0], [200.0, 200.0], 8.0);
    let both = panel.intersect(list);
    assert_eq!((both.lo, both.hi), (Vec2::splat(10.0), Vec2::splat(100.0)));
    // Left and bottom are the list's (soft), right and top the panel's (hard).
    assert_eq!(both.feather, Vec4::new(8.0, 8.0, 0.0, 0.0));
    assert_eq!(list.intersect(panel), both, "order does not matter");
}

#[test]
fn ndc_maps_bounds_and_feather_onto_the_frame() {
    let clip = UiClip {
        rect: Some(Scissor {
            x: 0,
            y: 0,
            w: 50,
            h: 100,
        }),
        feather: [10.0, 20.0, 0.0, 5.0],
        mask: None,
    };
    let (rect, feather) = clip.ndc(Vec2::new(100.0, 200.0));
    // Top-left origin: the scissor covers the left half, top half of the frame.
    assert_eq!(rect, [-1.0, 0.0, 0.0, 1.0]);
    let want = [0.2, 0.2, 0.0, 0.05];
    assert!(feather.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6));
    let (open, none) = UiClip::default().ndc(Vec2::splat(64.0));
    assert_eq!((open, none), (NO_CLIP, [0.0; 4]));
}

#[test]
fn a_clipped_away_rect_has_no_scissor() {
    let empty = soft([50.0, 50.0], [40.0, 80.0], 0.0);
    let state = Inherited {
        visible: true,
        alpha: 1.0,
        clip: Some(empty),
        mask: None,
    };
    assert_eq!(state.clip_on(Vec2::splat(100.0)), None);
    assert_eq!(state.visible_clip(1.0, Vec2::splat(100.0)), None);
}
