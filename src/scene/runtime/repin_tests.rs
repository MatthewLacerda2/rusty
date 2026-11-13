//! A marker hidden by its target's despawn shows again when it is pinned to a
//! target anew (#1264).

use glam::Vec3;

use crate::components::RectTransformComponent;
use crate::ecs::World;
use crate::scene::authoring::ui::layout::rect_transform::set_world_anchor;

#[test]
fn pinning_a_lost_marker_to_a_target_again_shows_it() {
    let mut w = World::new();
    let enemy = w.spawn("enemy".into());
    let marker = w.spawn("marker".into());
    let mut rect = RectTransformComponent::default();
    set_world_anchor(&mut rect, Some(enemy), Vec3::ZERO);
    w.set_rect_transform(marker, Some(rect));
    w.despawn(enemy);
    let mut rect = w.rect_transform(marker).unwrap().clone();
    assert!(rect.world_anchor.as_ref().unwrap().target_lost);
    set_world_anchor(&mut rect, Some(marker), Vec3::ZERO);
    assert!(
        !rect.world_anchor.unwrap().target_lost,
        "pinning it to a target again shows it"
    );
}
