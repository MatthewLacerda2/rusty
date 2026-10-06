//! Which light steers the realtime shadow sun (#809, #887): only a directional light
//! the frame draws live. A Baked directional light, or a live light of another type,
//! leaves the default sun in place. CPU only: `sun_direction` reads just the `Scene`.

use glam::{Quat, Vec3};

use super::sun_direction;
use crate::components::LightMode;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

fn default_sun() -> Vec3 {
    Vec3::new(-0.5, -1.0, -0.3).normalize()
}

/// Spawn a light primitive turned by `rotation`; return its id.
fn rotated_light(scene: &mut Scene, primitive: Primitive, rotation: Quat) -> u32 {
    let id = create_entity(scene, "Light", Some(primitive));
    scene.world.transform_mut(id).unwrap().rotation = rotation;
    id
}

#[test]
fn the_shadow_sun_follows_only_a_live_directional_light() {
    let mut scene = Scene::new();
    let sun_turn = Quat::from_rotation_x(-0.7) * Quat::from_rotation_y(0.4);
    let sun = rotated_light(&mut scene, Primitive::DirectionalLight, sun_turn);
    scene.world.light_mut(sun).unwrap().mode = LightMode::Baked;
    let lamp_turn = Quat::from_rotation_z(1.1);
    rotated_light(&mut scene, Primitive::PointLight, lamp_turn);

    let dir = sun_direction(&scene, false);
    assert!(
        dir.abs_diff_eq(default_sun(), 1e-6),
        "a Baked sun and a live point light keep the default sun, got {dir}"
    );

    scene.world.light_mut(sun).unwrap().mode = LightMode::Realtime;
    let live = (sun_turn * Vec3::NEG_Z).normalize();
    let dir = sun_direction(&scene, false);
    assert!(
        dir.abs_diff_eq(live, 1e-6),
        "a Realtime sun steers the shadows: want {live}, got {dir}"
    );
}
