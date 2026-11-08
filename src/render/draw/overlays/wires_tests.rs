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

/// The segments drawn in `tint`.
fn count(verts: &[Vertex], tint: [f32; 4]) -> usize {
    verts.iter().filter(|v| v.tangent == tint).count() / 2
}

fn segments(scene: &Scene, scope: ColliderScope) -> Vec<Vertex> {
    wire_vertices(
        scene,
        (&mut WireCache::default(), &DrawPoses::default()),
        (scope, Vec3::ZERO),
    )
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
        count(&segments(&scene, ColliderScope::Selected), COLLIDER),
        12
    );
}

/// A trigger draws paler, a debug line in its own colour, all in one buffer.
#[test]
fn triggers_and_debug_lines_draw_in_their_own_tints() {
    let mut scene = Scene::new();
    add_box(&mut scene, 0.0, false);
    add_box(&mut scene, 3.0, true);
    let red = [1.0, 0.0, 0.0, 1.0];
    scene.debug_lines.push(Vec3::ZERO, Vec3::Y, red, 1);
    let verts = segments(&scene, ColliderScope::All);
    assert_eq!((count(&verts, COLLIDER), count(&verts, TRIGGER)), (12, 12));
    assert_eq!(count(&verts, red), 1);
    let verts = segments(&scene, ColliderScope::Selected);
    assert_eq!(verts.len(), 2, "debug lines draw without any collider");
}

/// A frame never draws more collider segments than its cap, however many there are.
#[test]
fn the_cap_bounds_a_frame() {
    let mut scene = Scene::new();
    let boxes = MAX_COLLIDER_SEGMENTS / 12 + 10;
    for i in 0..boxes {
        add_box(&mut scene, i as f32, false);
    }
    let verts = segments(&scene, ColliderScope::All);
    assert_eq!(count(&verts, COLLIDER), MAX_COLLIDER_SEGMENTS);
}

/// A collider with no mesh (a CharacterController's body, a trigger) is placed
/// where the frame draws it, blended between its last two ticks as a mesh is
/// (#1877), so its outline never runs a tick ahead.
#[test]
fn a_wire_is_drawn_between_ticks() {
    let mut scene = Scene::new();
    let id = add_box(&mut scene, 0.0, false);
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    scene.world.transform_mut(id).expect("live").position = Vec3::X;
    poses.record(&scene);
    poses.set_alpha(0.25);
    let mut cache = WireCache::default();
    let verts = wire_vertices(
        &scene,
        (&mut cache, &poses),
        (ColliderScope::All, Vec3::ZERO),
    );
    let centre = verts.iter().map(|v| v.position[0]).sum::<f32>() / verts.len() as f32;
    assert!(
        (centre - 0.25).abs() < 1e-4,
        "drawn at the blend, not the tick"
    );
}
