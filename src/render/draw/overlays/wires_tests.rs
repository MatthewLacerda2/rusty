use super::*;
use crate::scene::{ColliderComponent, ColliderShape};

fn add_box(scene: &mut Scene, x: f32, trigger: bool) -> u32 {
    let id = scene.add_entity(format!("box{x}"));
    scene.world.transform_mut(id).expect("fresh").position = Vec3::new(x, 0.0, 0.0);
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box { size: Vec3::ONE };
    col.is_trigger = trigger;
    scene.world.set_collider(id, Some(col));
    id
}

fn count(groups: &[([f32; 4], Vec<Vec3>)], tint: [f32; 4]) -> usize {
    let group = groups.iter().find(|g| g.0 == tint);
    group.map_or(0, |g| g.1.len() / 2)
}

/// With `Colliders` off only the selection draws; on, every collider, the
/// selection first and the rest nearest the eye first.
#[test]
fn the_selection_draws_alone_until_every_collider_is_asked_for() {
    let mut scene = Scene::new();
    let far = add_box(&mut scene, 10.0, false);
    let near = add_box(&mut scene, 2.0, false);
    let picked = add_box(&mut scene, 20.0, false);
    assert!(collider_order(&scene, ColliderScope::Selected, Vec3::ZERO).is_empty());
    scene.selected_entity_id = Some(picked);
    let only = collider_order(&scene, ColliderScope::Selected, Vec3::ZERO);
    assert_eq!(only, vec![picked]);
    let all = collider_order(&scene, ColliderScope::All, Vec3::ZERO);
    assert_eq!(all, vec![picked, near, far]);
    assert_eq!(
        count(
            &wire_segments(&scene, ColliderScope::Selected, Vec3::ZERO),
            COLLIDER
        ),
        12
    );
}

/// A trigger draws paler, a debug line in its own colour, and both with the colliders.
#[test]
fn triggers_and_debug_lines_draw_in_their_own_tints() {
    let mut scene = Scene::new();
    add_box(&mut scene, 0.0, false);
    add_box(&mut scene, 3.0, true);
    let red = [1.0, 0.0, 0.0, 1.0];
    scene.debug_lines.push(Vec3::ZERO, Vec3::Y, red, 1);
    let groups = wire_segments(&scene, ColliderScope::All, Vec3::ZERO);
    assert_eq!(
        (count(&groups, COLLIDER), count(&groups, TRIGGER)),
        (12, 12)
    );
    assert_eq!(count(&groups, red), 1);
    let groups = wire_segments(&scene, ColliderScope::Selected, Vec3::ZERO);
    assert_eq!(groups.len(), 1, "debug lines draw without any collider");
}

/// A frame never draws more collider segments than its cap, however many there are.
#[test]
fn the_cap_bounds_a_frame() {
    let mut scene = Scene::new();
    let boxes = MAX_COLLIDER_SEGMENTS / 12 + 10;
    for i in 0..boxes {
        add_box(&mut scene, i as f32, false);
    }
    let groups = wire_segments(&scene, ColliderScope::All, Vec3::ZERO);
    assert_eq!(count(&groups, COLLIDER), MAX_COLLIDER_SEGMENTS);
}
