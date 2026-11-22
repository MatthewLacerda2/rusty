use glam::{Quat, Vec3};
use rusty_components::CameraComponent;
use rusty_navigation::test_support::add_floor;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

use super::*;

fn yard() -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, -10.0, 10.0, -10.0, 10.0);
    let cam = scene.add_entity("Main Camera".into());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    scene
        .world
        .set_world_position(cam, Vec3::new(1.0, 2.0, 3.0));
    let player = scene.add_entity("Player".into());
    scene
        .world
        .set_world_position(player, Vec3::new(0.0, 0.9, 0.0));
    scene
        .world
        .set_world_rotation(player, Quat::from_rotation_y(std::f32::consts::PI));
    scene
}

#[test]
fn cameras_then_the_spawn_then_navmesh_points_along_four_yaws() {
    let scene = yard();
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    assert!(views.warnings.is_empty(), "{:?}", views.warnings);
    assert_eq!(views.views.len(), 1 + 1 + 6 * YAWS.len());
    assert_eq!(views.views[0].label, "camera Main Camera");
    assert!(matches!(views.views[0].view, View::Camera(_)));
    let View::Pose { position, yaw, .. } = views.views[1].view else {
        panic!("the spawn is a pose");
    };
    assert_eq!(views.views[1].label, "spawn");
    assert!(
        (position.y - EYE_HEIGHT).abs() < 0.05,
        "eye over the floor: {position}"
    );
    assert!((yaw - 90.0).abs() < 0.01, "faces +Z: {yaw}");
    for (i, v) in views.views[2..].iter().enumerate() {
        let View::Pose {
            position,
            yaw,
            pitch,
        } = v.view
        else {
            panic!("a navmesh view is a pose");
        };
        assert_eq!(yaw, YAWS[i % 4]);
        assert_eq!(pitch, 0.0);
        assert!((position.y - EYE_HEIGHT).abs() < 0.05, "{position}");
        assert!(position.x.abs() <= 10.0 && position.z.abs() <= 10.0);
    }
    let at = |i: usize| match views.views[i].view {
        View::Pose { position, .. } => position,
        View::Camera(_) => unreachable!(),
    };
    assert_eq!(at(2), at(5));
    assert_ne!(at(2), at(6));
}

#[test]
fn two_picks_of_one_scene_are_the_same() {
    let scene = yard();
    let nav = NavigationGraph::from_scene(&scene);
    assert_eq!(pick(&scene, &nav), pick(&scene, &nav));
}

#[test]
fn no_navmesh_uses_the_cameras_alone_and_says_so() {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Cam".into());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    assert_eq!(views.views.len(), 1);
    assert_eq!(
        views.warnings,
        ["no navmesh: the scene's cameras are its only viewpoints"]
    );
}

#[test]
fn neither_camera_nor_navmesh_looks_along_z_from_the_bounds_centre() {
    let mut scene = Scene::new();
    for (name, x) in [("a", -4.0), ("b", 6.0)] {
        let id = scene.add_entity(name.into());
        scene.world.set_world_position(id, Vec3::new(x, 2.0, 0.0));
    }
    let nav = NavigationGraph::from_scene(&scene);
    let views = pick(&scene, &nav);
    let want = View::Pose {
        position: Vec3::new(1.0, 2.0, 0.0),
        yaw: 90.0,
        pitch: 0.0,
    };
    assert_eq!(
        views.views,
        [Viewpoint {
            label: "bounds centre".into(),
            view: want,
            at: Vec3::new(1.0, 2.0, 0.0),
            yaw: 90.0,
        }]
    );
    assert_eq!(views.warnings.len(), 1);
}

#[test]
fn a_camera_view_is_the_only_screen_camera_until_restored() {
    let mut scene = Scene::new();
    let ids: Vec<u32> = (0..2)
        .map(|i| {
            let id = scene.add_entity(format!("cam{i}"));
            scene.world.set_camera(id, Some(CameraComponent::default()));
            id
        })
        .collect();
    let mut camera = rusty_scene::Camera::new(Vec3::ZERO, 0.0, 0.0);
    let off = apply(&mut scene, &mut camera, &View::Camera(ids[1]));
    assert_eq!(off, [ids[0]]);
    assert_eq!(rusty_scene::camera::main_camera(&scene), Some(ids[1]));
    restore(&mut scene, &off);
    assert_eq!(rusty_scene::camera::main_camera(&scene), Some(ids[0]));
}

#[test]
fn a_pose_moves_the_main_camera() {
    let mut scene = Scene::new();
    let id = scene.add_entity("cam".into());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    let mut camera = rusty_scene::Camera::new(Vec3::ZERO, 0.0, 0.0);
    let to = Vec3::new(3.0, 1.7, -2.0);
    let view = View::Pose {
        position: to,
        yaw: 180.0,
        pitch: 0.0,
    };
    assert!(apply(&mut scene, &mut camera, &view).is_empty());
    assert!(scene.world.world_position(id).distance(to) < 1e-4);
    assert_eq!(camera.yaw, 180.0);
}
