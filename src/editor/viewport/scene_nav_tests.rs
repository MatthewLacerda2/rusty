//! Unit tests for the Scene view's input mapping (#745): Unity's button → gesture
//! table, scroll's two meanings, and the bounds frame-selected fits.

use super::*;
use crate::scene::authoring::{self, Primitive};

fn buttons(primary: bool, secondary: bool, middle: bool, alt: bool) -> Buttons {
    Buttons {
        primary,
        secondary,
        middle,
        alt,
    }
}

#[test]
fn unity_bindings_map_to_gestures() {
    assert_eq!(
        gesture(buttons(false, true, false, false)),
        Some(Gesture::Look)
    );
    assert_eq!(
        gesture(buttons(false, false, true, false)),
        Some(Gesture::Pan)
    );
    assert_eq!(
        gesture(buttons(false, false, true, true)),
        Some(Gesture::Pan)
    );
    assert_eq!(
        gesture(buttons(true, false, false, true)),
        Some(Gesture::Orbit)
    );
    assert_eq!(
        gesture(buttons(false, true, false, true)),
        Some(Gesture::Zoom)
    );
    // Plain LMB belongs to the tools (pick, gizmo, rect tool).
    assert_eq!(gesture(buttons(true, false, false, false)), None);
    assert_eq!(gesture(buttons(false, false, false, true)), None);
}

#[test]
fn scroll_zooms_unless_looking_where_it_sets_fly_speed() {
    let frame = NavFrame {
        scroll: POINTS_PER_STEP,
        ..Default::default()
    };
    let mut cam = SceneCamera::default();
    let (distance, speed) = (cam.distance, cam.fly_speed);
    apply(&mut cam, &frame);
    assert!(cam.distance < distance, "scroll up zooms in");
    assert_eq!(cam.fly_speed, speed);

    let mut cam = SceneCamera::default();
    let looking = NavFrame {
        gesture: Some(Gesture::Look),
        ..frame
    };
    apply(&mut cam, &looking);
    assert_eq!(cam.distance, distance, "no zoom while looking");
    assert!(cam.fly_speed > speed, "scroll up flies faster");
}

#[test]
fn only_looking_flies() {
    let fly = NavFrame {
        fly: Vec3::Z,
        dt: 1.0,
        ..Default::default()
    };
    let mut cam = SceneCamera::default();
    let eye = cam.position();
    apply(&mut cam, &fly);
    assert_eq!(cam.position(), eye, "WASD without RMB does nothing");
    apply(
        &mut cam,
        &NavFrame {
            gesture: Some(Gesture::Look),
            ..fly
        },
    );
    assert!((cam.position() - eye).dot(cam.forward()) > 0.0);
}

#[test]
fn drag_gestures_route_to_their_motion() {
    let base = NavFrame {
        motion: Vec2::new(40.0, 0.0),
        delta: Vec2::new(40.0, 0.0),
        viewport_height: 400.0,
        ..Default::default()
    };
    let start = SceneCamera::default();
    let mut orbit = start;
    apply(
        &mut orbit,
        &NavFrame {
            gesture: Some(Gesture::Orbit),
            ..base
        },
    );
    assert_eq!(orbit.pivot, start.pivot);
    assert!(orbit.yaw > start.yaw);
    let mut pan = start;
    apply(
        &mut pan,
        &NavFrame {
            gesture: Some(Gesture::Pan),
            ..base
        },
    );
    assert!((pan.pivot - start.pivot).dot(start.right()) < 0.0);
    let mut zoom = start;
    apply(
        &mut zoom,
        &NavFrame {
            gesture: Some(Gesture::Zoom),
            ..base
        },
    );
    assert!(
        zoom.distance < start.distance,
        "Alt+RMB drag right zooms in"
    );
}

#[test]
fn selection_bounds_cover_the_subtree() {
    let mut scene = Scene::new();
    let parent = authoring::create_entity(&mut scene, "Parent", Some(Primitive::Box));
    let child = authoring::create_entity(&mut scene, "Child", Some(Primitive::Box));
    scene.world.transform_mut(child).unwrap().position = Vec3::new(4.0, 0.0, 0.0);
    scene.set_parent(child, Some(parent)).unwrap();
    let (center, radius) = selection_bounds(&scene, parent).unwrap();
    // Unit boxes at x=0 and x=4: the union spans -0.5..4.5 in x.
    assert!(center.abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), 1e-4));
    assert!((radius - Vec3::new(5.0, 1.0, 1.0).length() * 0.5).abs() < 1e-4);

    // An empty marker frames a unit sphere at its position; a gone id frames nothing.
    let marker = authoring::create_entity(&mut scene, "Marker", None);
    scene.world.transform_mut(marker).unwrap().position = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(
        selection_bounds(&scene, marker),
        Some((Vec3::new(1.0, 2.0, 3.0), 1.0))
    );
    assert_eq!(selection_bounds(&scene, 9_999), None);
}
