use glam::Vec3;

use crate::authoring::ui::layout::rect_transform::set_world_anchor;
use crate::to_scene_data;
use rusty_components::{JointComponent, RectTransformComponent};
use rusty_scene::Scene;

#[test]
fn a_surviving_marker_targeting_a_non_survivor_ends_with_no_target() {
    let mut s = Scene::new();
    let enemy = s.add_entity("Enemy".to_string());
    let hud = s.add_entity("Hud".to_string());
    let mut rect = RectTransformComponent::default();
    set_world_anchor(&mut rect, Some(enemy), Vec3::Y);
    s.world.set_rect_transform(hud, Some(rect));
    let joint = JointComponent {
        connected_body: Some(enemy),
        ..JointComponent::default()
    };
    s.world.set_joint(hud, Some(joint));
    s.dont_destroy_on_load(hud);

    let mut level = Scene::new();
    let floor = level.add_entity("Floor".to_string());
    assert_eq!(floor, enemy);
    let survivors = s.load_survivors();
    crate::load::swap_in(
        &mut s,
        to_scene_data(&level),
        "rusty_1264_no_dir/level.scene",
        &survivors,
    )
    .unwrap();

    assert_eq!(
        s.find_entity_by_name("Floor"),
        Some(enemy),
        "the id is reused"
    );
    let anchor = s.world.rect_transform(hud).unwrap().world_anchor.clone();
    let anchor = anchor.unwrap();
    assert_eq!(anchor.target, None, "not the floor");
    assert!(
        anchor.target_lost,
        "its target was destroyed with the old scene"
    );
    assert_eq!(s.world.joint(hud).unwrap().connected_body, None);
}
