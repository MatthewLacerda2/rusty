//! A script moving a parent moves its children's collider bounds with it (#1157),
//! whether it writes through `Transform.Set*` or a tween: every writer lands its
//! Transform through one guard, which re-derives the subtree. Adding a collider in Play
//! sets its box at once, with nothing moving after.

use glam::Vec3;

use super::timers::{counted, rig, tick};
use crate::scene::authoring::defaults::default_collider;
use crate::scripting::manager::ScriptManager;

/// A collider child one unit along X from the script's entity; returns its id.
fn child_crate(m: &ScriptManager, parent: u32) -> u32 {
    let mut scene = m.scene.borrow_mut();
    let child = scene.add_entity("Wheel".to_string());
    scene.set_parent(child, Some(parent)).unwrap();
    scene.world.transform_mut(child).unwrap().position = Vec3::X;
    scene.world.set_collider(child, Some(default_collider()));
    child
}

/// The centre of `id`'s world AABB, and where `id` stands.
fn centre_and_stand(m: &ScriptManager, id: u32) -> (Vec3, Vec3) {
    let scene = m.scene.borrow();
    let (min, max) = scene.world.collider_bounds(id).unwrap();
    ((min + max) * 0.5, scene.world.world_position(id))
}

#[test]
fn set_position_on_a_parent_moves_the_childs_bounds() {
    let (m, parent) = rig("subtree_set", &counted(""));
    let child = child_crate(&m, parent);
    m.eval(&format!("Transform.SetPosition({parent}, 0, 7, 0)"))
        .unwrap();
    m.eval(&format!("Transform.SetRotation({parent}, 0, 90, 0)"))
        .unwrap();
    let (centre, stands) = centre_and_stand(&m, child);
    assert!(
        centre.abs_diff_eq(Vec3::new(0.0, 7.0, -1.0), 1e-4),
        "{centre}"
    );
    assert!(centre.abs_diff_eq(stands, 1e-4));
}

#[test]
fn a_tween_on_a_parent_moves_the_childs_bounds() {
    let body = "Tween.To(id, 'Transform.position', {4, 0, 0}, 2 / 60)";
    let (mut m, parent) = rig("subtree_tween", &counted(body));
    let child = child_crate(&m, parent);
    tick(&mut m, 4);
    let (centre, stands) = centre_and_stand(&m, child);
    assert!(
        centre.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-4),
        "{centre}"
    );
    assert!(centre.abs_diff_eq(stands, 1e-4));
}

#[test]
fn a_collider_added_in_play_reports_its_bounds_at_once() {
    // A static entity is never moved again, so nothing but adding it may set its box.
    let (m, id) = rig("collider_added", &counted(""));
    {
        let mut scene = m.scene.borrow_mut();
        scene.world.set_static(id, true);
        scene.world.transform_mut(id).unwrap().position = Vec3::new(5.0, 2.0, 0.0);
    }
    let read = m
        .eval(&format!(
            "Scene.AddComponent({id}, 'Collider') \
             local _, a, b, c, d, e, f = Physics.GetBounds({id}) \
             return string.format('%g %g %g %g %g %g', a, b, c, d, e, f)"
        ))
        .unwrap();
    assert_eq!(read.trim(), "4.5 1.5 -0.5 5.5 2.5 0.5");
}
