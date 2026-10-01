use glam::Vec2;

use super::*;

const PARENT: Vec2 = Vec2::new(800.0, 600.0);

/// A 200×100 box with its pivot at the centre, anchored at the parent's centre and
/// offset up-right — the default element, moved.
fn element() -> RectTransformComponent {
    let mut r = RectTransformComponent::default();
    r.size_delta = Vec2::new(200.0, 100.0);
    r.anchored_position = Vec2::new(50.0, 20.0);
    r
}

fn preset(x: AxisPreset, y: AxisPreset, set_pivot: bool, set_position: bool) -> AnchorPreset {
    AnchorPreset {
        x,
        y,
        set_pivot,
        set_position,
    }
}

fn close(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
    (a.0 - b.0).abs().max_element() < 1e-3 && (a.1 - b.1).abs().max_element() < 1e-3
}

#[test]
fn every_plain_preset_keeps_the_rect_where_it_is() {
    for x in AxisPreset::ALL {
        for y in AxisPreset::ALL {
            for set_pivot in [false, true] {
                let mut r = element();
                let before = r.layout_in(Vec2::ZERO, PARENT);
                apply_anchor_preset(&mut r, preset(x, y, set_pivot, false), PARENT);
                let after = r.layout_in(Vec2::ZERO, PARENT);
                assert!(close(before, after), "{x:?}/{y:?}: {before:?} → {after:?}");
                assert_eq!(AxisPreset::of(r.anchor_min.x, r.anchor_max.x), Some(x));
                assert_eq!(AxisPreset::of(r.anchor_min.y, r.anchor_max.y), Some(y));
            }
        }
    }
}

#[test]
fn shift_moves_the_pivot_to_the_preset_point() {
    let mut r = element();
    let p = preset(AxisPreset::Max, AxisPreset::Min, true, false);
    apply_anchor_preset(&mut r, p, PARENT);
    assert_eq!(r.pivot, Vec2::new(1.0, 0.0));
    // Stretch pivots centre on its axis.
    let p = preset(AxisPreset::Stretch, AxisPreset::Max, true, false);
    apply_anchor_preset(&mut r, p, PARENT);
    assert_eq!(r.pivot, Vec2::new(0.5, 1.0));
}

#[test]
fn alt_snaps_onto_the_anchors_and_stretch_fills() {
    // Top-left, pivot too: the box hangs from the parent's top-left corner.
    let mut r = element();
    let p = preset(AxisPreset::Min, AxisPreset::Max, true, true);
    apply_anchor_preset(&mut r, p, PARENT);
    let (min, size) = r.layout_in(Vec2::ZERO, PARENT);
    assert_eq!(size, Vec2::new(200.0, 100.0));
    assert_eq!(min, Vec2::new(0.0, 500.0));
    // Stretch both: fills the parent whatever it was before.
    let p = preset(AxisPreset::Stretch, AxisPreset::Stretch, false, true);
    apply_anchor_preset(&mut r, p, PARENT);
    assert!(close(r.layout_in(Vec2::ZERO, PARENT), (Vec2::ZERO, PARENT)));
    // Back from a stretch to a point keeps the stretched size.
    let p = preset(AxisPreset::Center, AxisPreset::Center, true, true);
    apply_anchor_preset(&mut r, p, PARENT);
    assert_eq!(r.size_delta, PARENT);
    assert_eq!(r.anchored_position, Vec2::ZERO);
}

#[test]
fn names_round_trip_per_axis() {
    for p in AxisPreset::ALL {
        for axis in 0..2 {
            assert_eq!(AxisPreset::from_name(p.name(axis), axis), Some(p));
        }
    }
    assert_eq!(AxisPreset::from_name("left", 1), None);
    assert_eq!(AxisPreset::from_name("top", 1), Some(AxisPreset::Max));
    assert_eq!(AxisPreset::of(0.2, 0.7), None);
}
