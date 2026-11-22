use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_navigation::test_support::add_floor;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

use super::*;

fn add_cameras(scene: &mut Scene, n: usize) -> Vec<u32> {
    (0..n)
        .map(|i| {
            let id = scene.add_entity(format!("cam{i}"));
            scene.world.set_camera(id, Some(CameraComponent::default()));
            id
        })
        .collect()
}

fn add_spawn(scene: &mut Scene) {
    let id = scene.add_entity("PlayerSpawn".into());
    scene.world.set_world_position(id, Vec3::new(2.0, 1.0, 2.0));
}

fn labels(views: &Views) -> Vec<&str> {
    views.views.iter().map(|v| v.label.as_str()).collect()
}

#[test]
fn past_the_cap_the_first_cameras_by_id_are_benched_and_the_rest_said() {
    let mut scene = Scene::new();
    add_floor(&mut scene, -10.0, 10.0, -10.0, 10.0);
    let ids = add_cameras(&mut scene, MAX_POINTS + 2);
    add_spawn(&mut scene);
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    let want: Vec<View> = ids[..MAX_POINTS]
        .iter()
        .map(|&id| View::Camera(id))
        .collect();
    let got: Vec<View> = views.views.iter().map(|v| v.view.clone()).collect();
    assert_eq!(got, want, "no spawn and no navmesh point past the cap");
    assert_eq!(views.warnings.len(), 1, "{:?}", views.warnings);
    let w = &views.warnings[0];
    assert!(w.starts_with("10 cameras, 8 benched"), "{w}");
    assert!(
        w.ends_with("left out: the other 2 cameras, the spawn and the navmesh points"),
        "{w}"
    );
}

#[test]
fn exactly_the_cap_of_cameras_says_what_it_left_out_and_only_what_the_scene_has() {
    let mut scene = Scene::new();
    add_floor(&mut scene, -10.0, 10.0, -10.0, 10.0);
    add_cameras(&mut scene, MAX_POINTS);
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    assert_eq!(views.views.len(), MAX_POINTS);
    let said = "8 cameras, 8 benched (at most 8 positions), left out: the navmesh points";
    assert_eq!(views.warnings, [said], "no spawn in the scene: none named");
    let mut bare = Scene::new();
    add_cameras(&mut bare, MAX_POINTS);
    let nav = NavigationGraph::from_scene(&bare);
    let views = pick(&bare, &nav);
    assert!(
        views.warnings.iter().all(|w| !w.contains("left out")),
        "{:?}",
        views.warnings
    );
}

#[test]
fn a_viewpoint_is_keyed_by_where_it_looks_from_not_only_its_label() {
    let mut scene = Scene::new();
    let ids = add_cameras(&mut scene, 2);
    for &id in &ids {
        scene.world.set_name(id, "Cam".into());
    }
    scene
        .world
        .set_world_position(ids[1], Vec3::new(4.0, 0.0, 0.0));
    let nav = NavigationGraph::from_scene(&scene);
    let keys: Vec<ViewKey> = pick(&scene, &nav)
        .views
        .iter()
        .map(Viewpoint::key)
        .collect();
    assert_eq!(keys[0].label, keys[1].label, "one name");
    assert_ne!(keys[0], keys[1], "two cameras");
    assert_eq!(
        (keys[1].camera, keys[1].at),
        (Some(ids[1]), [4.0, 0.0, 0.0])
    );
}

#[test]
fn cameras_and_the_spawn_leave_the_navmesh_what_is_left() {
    let mut scene = Scene::new();
    add_floor(&mut scene, -10.0, 10.0, -10.0, 10.0);
    add_cameras(&mut scene, 3);
    add_spawn(&mut scene);
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    // 3 cameras and the spawn leave 4 of the 8 positions: exactly the cap, so no warning.
    assert_eq!(views.views.len(), 3 + 1 + 4 * YAWS.len());
    assert!(views.warnings.is_empty(), "{:?}", views.warnings);
}

#[test]
fn a_spawn_alone_without_a_navmesh_is_the_one_view() {
    let mut scene = Scene::new();
    add_spawn(&mut scene);
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    assert_eq!(labels(&views), ["spawn"], "no bounds-centre view");
    assert_eq!(
        views.warnings,
        ["no navmesh: the spawn is its only viewpoint"]
    );
}

#[test]
fn a_spawn_and_cameras_without_a_navmesh_are_the_views_and_both_are_named() {
    let mut scene = Scene::new();
    add_cameras(&mut scene, 2);
    add_spawn(&mut scene);
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    assert_eq!(labels(&views), ["camera cam0", "camera cam1", "spawn"]);
    assert_eq!(
        views.warnings,
        ["no navmesh: the scene's cameras and its spawn are its only viewpoints"]
    );
}
