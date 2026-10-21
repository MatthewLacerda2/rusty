//! `local_lights` reads a light in world space (#1196): a spotlight on a child of
//! a moved, yawed parent (a flashlight in a hand) lights from where it is drawn,
//! along the way it is drawn pointing.

use glam::{Quat, Vec3};

use super::lights::local_lights;
use crate::components::LightComponent;
use crate::scene::{LightType, Scene};

#[test]
fn a_parented_spotlight_lights_from_its_world_pose() {
    let mut scene = Scene::new();
    let hand = scene.add_entity("Hand".to_string());
    {
        let mut t = scene.world.transform_mut(hand).unwrap();
        t.position = Vec3::new(10.0, 1.0, 0.0);
        t.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    }
    let lamp = scene.add_entity("Flashlight".to_string());
    scene.set_parent(lamp, Some(hand)).unwrap();
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 0.0, -2.0);
    let light = LightComponent {
        light_type: LightType::Spotlight,
        ..crate::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    let lights = local_lights(&scene, false);
    let (l, _) = lights.first().expect("one local light");
    // Yawed 90°, the parent's -Z is world -X: two units along it from the hand.
    let at = Vec3::from(l.position);
    assert!(at.abs_diff_eq(Vec3::new(8.0, 1.0, 0.0), 1e-4), "{at}");
    let dir = Vec3::from(l.direction);
    assert!(dir.abs_diff_eq(Vec3::NEG_X, 1e-4), "{dir}");
}
