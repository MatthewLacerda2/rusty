use super::{Member, DONT_DESTROY_ON_LOAD};
use crate::Scene;

/// A World of one scene (`base.scene`, root `a`) and an additive one (`b.scene`, root `b` with a
/// child), its fog density 0.5 against the base's.
fn two_scenes() -> (Scene, u32, u32, u32) {
    let mut scene = Scene::new();
    scene.set_playing(true);
    let a = scene.add_entity("A".into());
    let b = scene.add_entity("B".into());
    let child = scene.add_entity("Child".into());
    scene.set_parent(child, Some(b)).unwrap();
    let mut settings = scene.settings();
    settings.fog.density = 0.5;
    let base = Some("base.scene".to_string());
    scene.join_loaded(base, "b.scene".into(), settings, &[b]);
    (scene, a, b, child)
}

fn path(scene: &Scene, id: u32) -> Option<String> {
    scene.member_path(id, Some("base.scene"))
}

#[test]
fn one_scene_names_the_active_path_and_a_dead_id_nothing() {
    let mut scene = Scene::new();
    let a = scene.add_entity("A".into());
    assert_eq!(scene.member_of(a), Some(Member::Scene(None)));
    assert_eq!(path(&scene, a).as_deref(), Some("base.scene"));
    assert_eq!(scene.member_of(a + 1), None);
    assert_eq!(
        scene.loaded.paths(Some("base.scene")),
        [Some("base.scene".into())]
    );
}

#[test]
fn a_child_is_its_roots_and_a_spawned_root_joins_the_active_scene() {
    let (mut scene, a, b, child) = two_scenes();
    assert_eq!(path(&scene, a).as_deref(), Some("base.scene"));
    assert_eq!(path(&scene, child).as_deref(), Some("b.scene"));
    let spawned = scene.add_entity("Spawned".into());
    assert_eq!(path(&scene, spawned).as_deref(), Some("base.scene"));
    scene.set_active_scene(scene.loaded.handle_of("b.scene").unwrap());
    assert_eq!(
        path(&scene, spawned).as_deref(),
        Some("base.scene"),
        "it joined the scene active when it was spawned"
    );
    let later = scene.add_entity("Later".into());
    assert_eq!(path(&scene, later).as_deref(), Some("b.scene"));
    scene.set_parent(a, Some(b)).unwrap();
    assert_eq!(path(&scene, a).as_deref(), Some("b.scene"), "moved under b");
}

#[test]
fn dont_destroy_on_load_is_a_membership_of_its_own() {
    let (mut scene, _, b, child) = two_scenes();
    scene.dont_destroy_on_load(b);
    assert_eq!(path(&scene, child).as_deref(), Some(DONT_DESTROY_ON_LOAD));
}

#[test]
fn the_active_scenes_settings_win_and_come_back() {
    let (mut scene, ..) = two_scenes();
    let base_fog = scene.fog.density;
    let b = scene.loaded.handle_of("b.scene").unwrap();
    assert_eq!(scene.set_active_scene(b).as_deref(), Some("b.scene"));
    assert_eq!(scene.fog.density, 0.5);
    let base = scene.loaded.handle_of("base.scene").unwrap();
    scene.set_active_scene(base);
    assert_eq!(scene.fog.density, base_fog);
}

#[test]
fn an_unload_queues_its_roots_destroy_and_the_last_scene_stays() {
    let (mut scene, a, b, _) = two_scenes();
    let handle = scene.loaded.handle_of("b.scene").unwrap();
    assert_eq!(
        scene.unload_scene(handle).unwrap().as_deref(),
        Some("base.scene")
    );
    assert_eq!(
        scene.take_pending_destroys(),
        [b],
        "the root only: its subtree goes with it"
    );
    assert_eq!(scene.loaded.unloads(), 1);
    assert_eq!(scene.loaded.paths(None), [Some("base.scene".into())]);
    let base = scene.loaded.handle_of("base.scene").unwrap();
    assert!(
        scene.unload_scene(base).is_err(),
        "the last loaded scene stays"
    );
    assert_eq!(path(&scene, a).as_deref(), Some("base.scene"));
}

#[test]
fn unloading_the_active_scene_hands_over_to_the_first_left() {
    let (mut scene, a, ..) = two_scenes();
    let b = scene.loaded.handle_of("b.scene").unwrap();
    scene.set_active_scene(b);
    let base = scene.loaded.handle_of("base.scene").unwrap();
    assert_eq!(
        scene.unload_scene(base).unwrap().as_deref(),
        Some("b.scene")
    );
    assert_eq!(scene.take_pending_destroys(), [a]);
    assert_eq!(scene.fog.density, 0.5, "b's settings stay the World's");
}
