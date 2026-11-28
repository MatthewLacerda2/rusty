use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_navigation::NavigationGraph;
use rusty_scene::camera::main_camera;
use rusty_scene::Scene;

use super::*;

/// Three screen cameras, as a scene is saved; then a `Start` turns all but the first off,
/// the way `games/city/scripts/viewpoints.lua` keeps Play on one camera.
fn played() -> (Scene, Vec<u32>, Vec<u32>) {
    let mut scene = Scene::new();
    for i in 0..3 {
        let id = scene.add_entity(format!("cam{i}"));
        scene.world.set_camera(id, Some(CameraComponent::default()));
    }
    let authored = screen_cameras(&scene);
    for &id in &authored[1..] {
        scene.world.set_active(id, false);
    }
    let ids = authored.clone();
    (scene, authored, ids)
}

#[test]
fn a_camera_a_script_turns_off_at_start_is_still_a_viewpoint() {
    let (scene, authored, ids) = played();
    let nav = NavigationGraph::from_scene(&scene);
    assert_eq!(
        pick(&scene, &nav).views.len(),
        1,
        "the live world holds one"
    );
    let views = pick_from(&scene, &nav, authored);
    let cameras: Vec<_> = views.views.iter().map(|v| v.view.clone()).collect();
    assert_eq!(
        cameras,
        ids.iter().map(|&id| View::Camera(id)).collect::<Vec<_>>()
    );
}

#[test]
fn its_view_wakes_it_alone_and_restore_puts_it_back_to_sleep() {
    let (mut scene, _, ids) = played();
    let mut camera = rusty_scene::Camera::new(Vec3::ZERO, 0.0, 0.0);
    let mut switched = Switched::default();
    for _ in 0..3 {
        // A pose is held after every step: applying again changes nothing more.
        apply(
            &mut scene,
            &mut camera,
            &View::Camera(ids[2]),
            &mut switched,
        );
    }
    assert_eq!(rusty_scene::camera::screen_cameras(&scene), [ids[2]]);
    restore(&mut scene, switched);
    assert_eq!(
        main_camera(&scene),
        Some(ids[0]),
        "Play's own camera is back"
    );
    assert!(
        !scene.world.is_active_self(ids[2]),
        "the script's off is kept"
    );
    assert!(scene.world.is_active_self(ids[0]));
}
