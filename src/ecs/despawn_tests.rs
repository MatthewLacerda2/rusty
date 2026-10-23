//! What a despawn clears in the World (#1264): component references to the dead
//! id go, each to its component's `None`, and a marker that followed it is hidden
//! but stays active.

use crate::components::ui::rect_transform::WorldAnchor;
use crate::components::{JointComponent, RectTransformComponent};
use crate::ecs::World;

#[test]
fn a_joint_to_a_despawned_body_holds_to_the_world() {
    let mut w = World::new();
    let body = w.spawn("body".into());
    let other = w.spawn("other".into());
    let (to_body, to_other) = (w.spawn("a".into()), w.spawn("b".into()));
    for (id, target) in [(to_body, body), (to_other, other)] {
        let joint = JointComponent {
            connected_body: Some(target),
            ..JointComponent::default()
        };
        w.set_joint(id, Some(joint));
    }
    w.despawn(body);
    assert_eq!(w.joint(to_body).unwrap().connected_body, None);
    assert_eq!(w.joint(to_other).unwrap().connected_body, Some(other));
}

#[test]
fn a_marker_whose_target_is_despawned_hides() {
    let mut w = World::new();
    let enemy = w.spawn("enemy".into());
    let marker = w.spawn("marker".into());
    let rect = RectTransformComponent {
        world_anchor: Some(WorldAnchor {
            target: Some(enemy),
            ..WorldAnchor::default()
        }),
        ..RectTransformComponent::default()
    };
    w.set_rect_transform(marker, Some(rect));
    let bystander = w.spawn("bystander".into());
    w.despawn(bystander);
    let anchor = |w: &World| {
        w.rect_transform(marker)
            .unwrap()
            .world_anchor
            .clone()
            .unwrap()
    };
    assert!(!anchor(&w).target_lost, "only its own target hides it");
    w.despawn(enemy);
    assert_eq!(anchor(&w).target, None);
    assert!(anchor(&w).target_lost, "hidden, not a fixed world point");
    assert!(w.is_active(marker), "its scripts keep running");

    let mut rect = w.rect_transform(marker).unwrap().clone();
    crate::scene::authoring::rect_transform::set_world_anchor(&mut rect, None, glam::Vec3::ZERO);
    assert!(
        !rect.world_anchor.unwrap().target_lost,
        "pinning it again shows it"
    );
}
