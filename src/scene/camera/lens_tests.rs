//! A lens that skipped the authoring ops (a scene file, a prefab) is held where the
//! view camera copies it, by the ops' own bounds, and draws a finite projection
//! (#1359).

use super::*;
use crate::components::CameraComponent;
use crate::scene::authoring::default_camera;
use crate::scene::authoring::rendering::camera as ops;
use crate::scene::serialize::{apply_scene_data, to_scene_data};

/// The view camera synced from a scene whose one camera is `c`.
fn synced(c: CameraComponent) -> (Camera, Scene) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Cam".to_string());
    scene.world.set_camera(id, Some(c));
    let mut camera = Camera::new(Vec3::ZERO, -90.0, 0.0);
    sync_lens_from_scene(&mut camera, &scene, true);
    (camera, scene)
}

/// The view camera of a scene whose camera's saved JSON is `patch`ed, then loaded
/// back the way a hand-edited `.scene` file is.
fn loaded(patch: serde_json::Value) -> Camera {
    let (_, mut scene) = synced(default_camera());
    let mut json = serde_json::to_value(to_scene_data(&scene)).unwrap();
    let lens = json["entities"][0]["camera"].as_object_mut().unwrap();
    lens.extend(patch.as_object().unwrap().clone());
    apply_scene_data(&mut scene, serde_json::from_value(json).unwrap());
    let mut camera = Camera::new(Vec3::ZERO, -90.0, 0.0);
    sync_lens_from_scene(&mut camera, &scene, true);
    // The entity view (render stack, Game tab) holds the lens the same way.
    let entity = camera_for_entity(&camera, &scene, scene.world.ids_with_camera()[0]);
    assert_eq!(
        entity.map(|c| (c.fov, c.near, c.far)),
        Some((camera.fov, camera.near, camera.far))
    );
    camera
}

fn assert_finite(cam: &Camera) {
    let m = cam.build_view_projection(16.0 / 9.0);
    assert!(m.is_finite(), "near {} far {}: {m:?}", cam.near, cam.far);
    assert!(cam.near >= ops::MIN_NEAR && cam.far > cam.near);
}

#[test]
fn a_zero_near_plane_from_a_file_draws() {
    let cam = loaded(serde_json::json!({ "near": 0.0 }));
    assert_finite(&cam);
    assert_eq!(cam.near, ops::MIN_NEAR);
}

#[test]
fn a_far_plane_below_the_near_one_is_pushed_out() {
    let cam = loaded(serde_json::json!({ "near": 5.0, "far": 1.0 }));
    assert_finite(&cam);
    assert_eq!((cam.near, cam.far), (5.0, 5.0 + ops::MIN_CLIP_GAP));
}

#[test]
fn an_overflowing_far_plane_draws() {
    // 1e39 overflows f32: it loads as infinity, which takes the far plane's floor.
    let cam = loaded(serde_json::json!({ "far": 1e39 }));
    assert_finite(&cam);
    assert_eq!(cam.far, default_camera().near + ops::MIN_CLIP_GAP);
}

#[test]
fn a_huge_finite_far_plane_draws() {
    // near * far overflows f32 in the perspective build without the cap.
    let cam = loaded(serde_json::json!({ "near": 10.0, "far": 1e38 }));
    assert_finite(&cam);
    assert_eq!(cam.far, f32::MAX / 10.0);
}

#[test]
fn a_huge_near_plane_draws() {
    let cam = loaded(serde_json::json!({ "near": 1e20, "far": 1e30 }));
    assert_finite(&cam);
    assert_eq!(cam.near, MAX_LENS_EXTENT);
}

#[test]
fn a_nan_or_infinite_fov_takes_the_ops_bound() {
    for (fov, held) in [(f32::NAN, ops::MIN_FOV), (f32::INFINITY, ops::MAX_FOV)] {
        let (cam, _) = synced(CameraComponent {
            fov,
            ..default_camera()
        });
        assert_finite(&cam);
        assert_eq!(cam.fov, held, "{fov}");
        assert_eq!(held_fov(fov), held);
    }
}

#[test]
fn a_zero_orthographic_size_takes_the_floor() {
    let ortho = |size| Projection::Orthographic { size };
    let (cam, _) = synced(CameraComponent {
        projection: ortho(0.0),
        ..default_camera()
    });
    assert_finite(&cam);
    assert_eq!(cam.projection, ortho(ops::MIN_ORTHO_SIZE));
}

#[test]
fn a_lens_the_ops_wrote_is_unchanged() {
    let c = CameraComponent {
        fov: 75.0,
        near: 0.01,
        far: 5000.0,
        ..default_camera()
    };
    let lens = held_lens(&c);
    assert_eq!((lens.fov, lens.near, lens.far), (75.0, 0.01, 5000.0));
    assert_eq!(lens.projection, c.projection);
}
