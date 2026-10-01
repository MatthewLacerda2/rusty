//! Row / column layout-group tests (#421): force-expand, preferred sizes with
//! alignment, the min→preferred lerp, flexible weights, uncontrolled children,
//! exclusions and nesting. Rects are relative to the panel's bottom-left.

use glam::{Vec2, Vec4};

use super::super::driven_by_group;
use super::super::fixture::{assert_rect, preferred, Fixture};
use crate::components::{LayoutElementComponent, LayoutGroupComponent, LayoutKind, TextAlignment};

fn group(kind: LayoutKind) -> LayoutGroupComponent {
    LayoutGroupComponent {
        kind,
        ..LayoutGroupComponent::default()
    }
}

fn no_expand(kind: LayoutKind, align: TextAlignment) -> LayoutGroupComponent {
    LayoutGroupComponent {
        child_force_expand_width: false,
        child_force_expand_height: false,
        child_alignment: align,
        ..group(kind)
    }
}

#[test]
fn a_row_force_expands_its_children_inside_the_padding() {
    let g = LayoutGroupComponent {
        padding: Vec4::splat(10.0),
        spacing: Vec2::new(10.0, 0.0),
        ..group(LayoutKind::Horizontal)
    };
    let mut f = Fixture::new(Vec2::new(300.0, 100.0), g);
    let kids: Vec<u32> = (0..3).map(|_| f.child(f.panel, None, Vec2::ZERO)).collect();
    let w = (300.0 - 20.0 - 20.0) / 3.0;
    for (i, &k) in kids.iter().enumerate() {
        assert_rect(f.rect(k), [10.0 + i as f32 * (w + 10.0), 10.0], [w, 80.0]);
    }
}

#[test]
fn preferred_sizes_are_aligned_when_nothing_is_flexible() {
    let g = no_expand(LayoutKind::Horizontal, TextAlignment::MiddleCenter);
    let mut f = Fixture::new(Vec2::new(300.0, 100.0), g);
    let a = f.child(f.panel, preferred(Some(50.0), Some(40.0)), Vec2::ZERO);
    let b = f.child(f.panel, preferred(Some(70.0), Some(20.0)), Vec2::ZERO);
    // 50 + 70 + 0 spacing leaves 180 spare; centred → starts at 90.
    assert_rect(f.rect(a), [90.0, 30.0], [50.0, 40.0]);
    assert_rect(f.rect(b), [140.0, 40.0], [70.0, 20.0]);
}

#[test]
fn a_short_row_lerps_children_from_min_toward_preferred() {
    let g = no_expand(LayoutKind::Horizontal, TextAlignment::TopLeft);
    let mut f = Fixture::new(Vec2::new(120.0, 10.0), g);
    let le = LayoutElementComponent {
        min_width: Some(20.0),
        preferred_width: Some(100.0),
        ..LayoutElementComponent::default()
    };
    let a = f.child(f.panel, Some(le.clone()), Vec2::ZERO);
    let b = f.child(f.panel, Some(le), Vec2::ZERO);
    // Totals min 40, preferred 200; 120 is halfway → 60 each.
    assert_rect(f.rect(a), [0.0, 10.0], [60.0, 0.0]);
    assert_rect(f.rect(b), [60.0, 10.0], [60.0, 0.0]);
}

#[test]
fn surplus_is_shared_by_flexible_weight() {
    let g = no_expand(LayoutKind::Horizontal, TextAlignment::TopLeft);
    let mut f = Fixture::new(Vec2::new(400.0, 10.0), g);
    let weight = |w: f32| {
        Some(LayoutElementComponent {
            flexible_width: Some(w),
            ..LayoutElementComponent::default()
        })
    };
    let a = f.child(f.panel, weight(1.0), Vec2::ZERO);
    let b = f.child(f.panel, weight(3.0), Vec2::ZERO);
    assert_rect(f.rect(a), [0.0, 10.0], [100.0, 0.0]);
    assert_rect(f.rect(b), [100.0, 10.0], [300.0, 0.0]);
}

#[test]
fn a_column_stacks_top_down() {
    let g = LayoutGroupComponent {
        spacing: Vec2::new(0.0, 5.0),
        ..no_expand(LayoutKind::Vertical, TextAlignment::TopLeft)
    };
    let mut f = Fixture::new(Vec2::new(100.0, 200.0), g);
    let a = f.child(f.panel, preferred(Some(40.0), Some(30.0)), Vec2::ZERO);
    let b = f.child(f.panel, preferred(Some(60.0), Some(50.0)), Vec2::ZERO);
    assert_rect(f.rect(a), [0.0, 170.0], [40.0, 30.0]);
    assert_rect(f.rect(b), [0.0, 115.0], [60.0, 50.0]);
}

#[test]
fn an_uncontrolled_axis_keeps_the_childs_own_size() {
    let g = LayoutGroupComponent {
        control_child_width: false,
        control_child_height: false,
        child_alignment: TextAlignment::BottomRight,
        ..group(LayoutKind::Horizontal)
    };
    let mut f = Fixture::new(Vec2::new(200.0, 100.0), g);
    let a = f.child(f.panel, None, Vec2::new(30.0, 20.0));
    let b = f.child(f.panel, None, Vec2::new(50.0, 40.0));
    // Force-expand shares the 120 spare equally: slots 30 + 60 and 50 + 60; each
    // child keeps its size at its slot's lower-right.
    assert_rect(f.rect(a), [60.0, 0.0], [30.0, 20.0]);
    assert_rect(f.rect(b), [150.0, 0.0], [50.0, 40.0]);
}

#[test]
fn ignored_and_inactive_children_keep_their_own_anchors() {
    let g = group(LayoutKind::Horizontal);
    let mut f = Fixture::new(Vec2::new(200.0, 100.0), g);
    let ignore = LayoutElementComponent {
        ignore_layout: true,
        ..LayoutElementComponent::default()
    };
    let ignored = f.child(f.panel, Some(ignore), Vec2::new(10.0, 10.0));
    let hidden = f.child(f.panel, None, Vec2::new(10.0, 10.0));
    f.scene.world.set_active(hidden, false);
    let only = f.child(f.panel, None, Vec2::ZERO);
    assert_rect(f.rect(only), [0.0, 0.0], [200.0, 100.0]);
    // Centred by their own point anchors, 10×10.
    assert_rect(f.rect(ignored), [95.0, 45.0], [10.0, 10.0]);
    assert_rect(f.rect(hidden), [95.0, 45.0], [10.0, 10.0]);
    // Only the arranged child is driven (the editor shows it read-only, #423).
    let w = &f.scene.world;
    assert!(driven_by_group(w, only));
    assert!(!driven_by_group(w, ignored) && !driven_by_group(w, hidden));
    assert!(
        !driven_by_group(w, f.panel),
        "the panel's parent has no group"
    );
}

#[test]
fn a_nested_group_reports_its_childrens_preferred_size() {
    let outer = no_expand(LayoutKind::Vertical, TextAlignment::TopLeft);
    let mut f = Fixture::new(Vec2::new(300.0, 300.0), outer);
    let row = f.child(f.panel, None, Vec2::ZERO);
    let inner = LayoutGroupComponent {
        spacing: Vec2::new(5.0, 0.0),
        ..no_expand(LayoutKind::Horizontal, TextAlignment::TopLeft)
    };
    f.scene.world.set_layout_group(row, Some(inner));
    let a = f.child(row, preferred(Some(30.0), Some(20.0)), Vec2::ZERO);
    let b = f.child(row, preferred(Some(30.0), Some(25.0)), Vec2::ZERO);
    assert_rect(f.rect(row), [0.0, 275.0], [65.0, 25.0]);
    assert_rect(f.rect(a), [0.0, 280.0], [30.0, 20.0]);
    assert_rect(f.rect(b), [35.0, 275.0], [30.0, 25.0]);
}

#[test]
fn rect_of_agrees_with_the_full_pass_under_a_group() {
    let mut f = Fixture::new(Vec2::new(300.0, 100.0), group(LayoutKind::Vertical));
    let row = f.child(f.panel, None, Vec2::ZERO);
    f.scene
        .world
        .set_layout_group(row, Some(group(LayoutKind::Horizontal)));
    let leaf = f.child(row, None, Vec2::ZERO);
    let _ = f.child(row, None, Vec2::ZERO);
    let full = super::super::UiLayout::compute(&f.scene.world, super::super::fixture::HD);
    let one = super::super::rect_of(&f.scene.world, leaf, super::super::fixture::HD);
    assert_eq!(full.get(leaf).copied(), one);
}
