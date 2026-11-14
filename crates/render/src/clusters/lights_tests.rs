//! `local_lights` reads a light in world space (#1196): a spotlight on a child of
//! a moved, yawed parent (a flashlight in a hand) lights from where it is drawn,
//! along the way it is drawn pointing.

use glam::{Quat, Vec3};

use super::lights::local_lights;
use rusty_components::LightComponent;
use rusty_scene::{LightType, Scene};

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
        ..rusty_scene::authoring::default_light()
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

#[test]
fn a_bake_capture_takes_no_shadow_fade() {
    let mut scene = Scene::new();
    let lamp = scene.add_entity("Lamp".to_string());
    let light = LightComponent {
        light_type: LightType::Point,
        shadow_fade_distance: 20.0,
        ..rusty_scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    let fade = |capture| local_lights(&scene, capture)[0].0.shadow_fade;
    assert_eq!((fade(false), fade(true)), (20.0, 0.0));
}

/// An Area light's capture stand-in (#1674) is its own kind (#1699): shadowed
/// through one frustum like a spot, but fading as the Lambert `cos θ`, which the
/// spotlight's squared cone would not give it.
#[test]
fn an_area_light_stands_in_as_its_own_one_frustum_kind() {
    use super::lights::{LocalLight, KIND_AREA};
    let light = LightComponent {
        light_type: LightType::Area,
        ..rusty_scene::authoring::default_light()
    };
    let l = LocalLight::new(Vec3::ZERO, Vec3::NEG_Z, &light).expect("a local light");
    assert_eq!(l.kind, KIND_AREA);
    assert!(l.one_frustum());
    // Touching the panel, its stand-in gives the true limit, `π × intensity`,
    // not one point's `intensity × area / ε²` (#1699).
    let touching = l.intensity * rusty_components::falloff_near(0.0, l.range, l.near);
    let limit = std::f32::consts::PI * light.intensity;
    assert!(
        (touching - limit).abs() < limit * 1e-5,
        "{touching} vs {limit}"
    );
    let point = LightComponent {
        light_type: LightType::Point,
        ..rusty_scene::authoring::default_light()
    };
    let p = LocalLight::new(Vec3::ZERO, Vec3::NEG_Z, &point).unwrap();
    assert!(!p.one_frustum() && p.near == 0.0);
}
