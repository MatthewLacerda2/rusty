//! The scene pieces the atlas's GPU tests share: a dark scene, and a light that
//! casts or not.

use glam::{Quat, Vec3};

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// A dark scene: no sky, sun or ambient.
pub(super) fn dark() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    scene
}

/// Add a `kind` light at `at`, pointing along `dir`, casting when `shadows`.
pub(super) fn lamp(scene: &mut Scene, kind: Primitive, at: Vec3, dir: Vec3, shadows: bool) {
    let id = create_entity(scene, "Lamp", Some(kind));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation) = (at, Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize()));
    drop(t);
    let mut light = scene.world.light(id).unwrap().clone();
    (light.range, light.intensity, light.outer_cone) = (10.0, 30.0, 60.0);
    light.cast_shadows = shadows;
    scene.world.set_light(id, Some(light));
}
