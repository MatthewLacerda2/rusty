//! Tests for the UI raycast (#420): draw order, canvas sort order, raycast_target,
//! active, CanvasGroup.blocks_raycasts, RectMask and Mask clipping and rotated rects.

use glam::{Quat, Vec2};

use super::raycast;
use crate::components::{CanvasComponent, CanvasGroupComponent, MaskComponent, RectMaskComponent};
use crate::ui::events::fixture::{panel, scene, SCREEN};
use crate::ui::UiLayout;

fn hit(scene: &crate::scene::Scene, x: f32, y: f32) -> Option<u32> {
    let layout = UiLayout::compute(&scene.world, SCREEN);
    raycast(&scene.world, &layout, Vec2::new(x, y))
}

#[test]
fn the_top_most_graphic_wins_and_empty_space_misses() {
    let (mut s, canvas) = scene();
    let back = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let front = panel(&mut s, canvas, Vec2::splat(50.0), Vec2::splat(100.0));
    assert_eq!(hit(&s, 25.0, 25.0), Some(back));
    assert_eq!(
        hit(&s, 75.0, 75.0),
        Some(front),
        "a later sibling is on top"
    );
    assert_eq!(hit(&s, 500.0, 500.0), None);
    // A second canvas with a higher sort order is hit first, whatever its order.
    let top = s.add_entity("Top".to_string());
    let c = CanvasComponent {
        sort_order: 5,
        ..CanvasComponent::default()
    };
    s.world.set_canvas(top, Some(c));
    let over = panel(&mut s, top, Vec2::ZERO, Vec2::splat(60.0));
    s.world.canvas_mut(canvas).expect("canvas").sort_order = 9;
    assert_eq!(hit(&s, 55.0, 55.0), Some(front), "sort order 9 beats 5");
    s.world.canvas_mut(canvas).expect("canvas").sort_order = 0;
    assert_eq!(hit(&s, 55.0, 55.0), Some(over));
}

#[test]
fn non_targets_inactive_and_non_blocking_groups_let_the_pointer_through() {
    let (mut s, canvas) = scene();
    let back = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    let front = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    s.world.image_mut(front).expect("image").raycast_target = false;
    assert_eq!(hit(&s, 10.0, 10.0), Some(back));
    s.world.image_mut(front).expect("image").raycast_target = true;
    s.world.set_active(front, false);
    assert_eq!(hit(&s, 10.0, 10.0), Some(back));
    s.world.set_active(front, true);
    let group = CanvasGroupComponent {
        blocks_raycasts: false,
        ..CanvasGroupComponent::default()
    };
    s.world.set_canvas_group(front, Some(group));
    assert_eq!(hit(&s, 10.0, 10.0), Some(back));
    // A transparent group still blocks (alpha does not matter).
    let group = CanvasGroupComponent {
        alpha: 0.0,
        ..CanvasGroupComponent::default()
    };
    s.world.set_canvas_group(front, Some(group));
    assert_eq!(hit(&s, 10.0, 10.0), Some(front));
    // An inactive canvas hides its whole tree.
    s.world.set_active(canvas, false);
    assert_eq!(hit(&s, 10.0, 10.0), None);
}

#[test]
fn a_rect_mask_clips_what_it_contains() {
    let (mut s, canvas) = scene();
    let mask = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    s.world.image_mut(mask).expect("image").raycast_target = false;
    s.world
        .set_rect_mask(mask, Some(RectMaskComponent::default()));
    let child = panel(&mut s, mask, Vec2::ZERO, Vec2::splat(300.0));
    assert_eq!(hit(&s, 50.0, 50.0), Some(child));
    assert_eq!(hit(&s, 150.0, 150.0), None, "outside the mask");
}

#[test]
fn a_mask_clips_its_children_but_not_itself() {
    let (mut s, canvas) = scene();
    let frame = panel(&mut s, canvas, Vec2::ZERO, Vec2::splat(100.0));
    s.world.set_mask(frame, Some(MaskComponent::default()));
    let map = panel(&mut s, frame, Vec2::ZERO, Vec2::splat(300.0));
    assert_eq!(hit(&s, 50.0, 50.0), Some(map));
    assert_eq!(hit(&s, 150.0, 150.0), None, "a masked-out child is not hit");
    s.world.image_mut(map).expect("image").raycast_target = false;
    assert_eq!(
        hit(&s, 50.0, 50.0),
        Some(frame),
        "the mask graphic still is"
    );
}

#[test]
fn a_rotated_rect_is_hit_by_its_quad_not_its_bounds() {
    let (mut s, canvas) = scene();
    let diamond = panel(&mut s, canvas, Vec2::splat(100.0), Vec2::splat(100.0));
    let mut rt = s.world.rect_transform_mut(diamond).expect("rect");
    rt.pivot = Vec2::splat(0.5);
    rt.anchored_position = Vec2::splat(150.0);
    drop(rt);
    s.world.transform_mut(diamond).expect("transform").rotation =
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
    assert_eq!(hit(&s, 150.0, 150.0), Some(diamond), "the centre");
    assert_eq!(
        hit(&s, 150.0, 215.0),
        Some(diamond),
        "a tip past the old edge"
    );
    assert_eq!(hit(&s, 105.0, 105.0), None, "a cut-off corner");
}
