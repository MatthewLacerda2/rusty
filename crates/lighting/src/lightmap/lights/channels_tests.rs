//! Shadowmask channels (#1695): an overlap colouring of the `Mixed` lights' reach,
//! four to a texel, the rest baked as `Baked`.

use glam::Vec3;

use super::assign_channels;
use rusty_scene::authoring::default_light;
use rusty_scene::lighting::MixedLightingMode;
use rusty_scene::Scene;
use rusty_components::{LightComponent, LightMode, LightType};

fn shadowmask() -> Scene {
    let mut scene = Scene::new();
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::Shadowmask;
    scene
}

fn lamp(scene: &mut Scene, at: Vec3, light_type: LightType, mode: LightMode) -> u32 {
    let id = scene.add_entity("Lamp".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = at;
    }
    let light = LightComponent {
        light_type,
        mode,
        cast_shadows: true,
        range: 5.0,
        ..default_light()
    };
    scene.world.set_light(id, Some(light));
    id
}

fn point(scene: &mut Scene, x: f32) -> u32 {
    lamp(scene, Vec3::X * x, LightType::Point, LightMode::Mixed)
}

#[test]
fn baked_indirect_masks_nothing() {
    let mut scene = shadowmask();
    point(&mut scene, 0.0);
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::BakedIndirect;
    assert_eq!(assign_channels(&scene), Default::default());
}

#[test]
fn a_fifth_overlapping_light_falls_back_to_baked() {
    let mut scene = shadowmask();
    let lamps: Vec<u32> = (0..5).map(|k| point(&mut scene, k as f32)).collect();
    let got = assign_channels(&scene);
    for (k, id) in lamps[..4].iter().enumerate() {
        assert_eq!(got.channels.get(id), Some(&(k as u8)), "lamp {k}");
    }
    assert_eq!(
        got.fell_back.iter().copied().collect::<Vec<_>>(),
        [lamps[4]]
    );
}

#[test]
fn lights_that_never_meet_share_a_channel_and_the_sun_meets_all() {
    let mut scene = shadowmask();
    let near = point(&mut scene, 0.0);
    let far = point(&mut scene, 100.0);
    let sun = lamp(
        &mut scene,
        Vec3::ZERO,
        LightType::Directional,
        LightMode::Mixed,
    );
    let got = assign_channels(&scene);
    assert_eq!(got.channels[&sun], 0, "directional lights colour first");
    assert_eq!((got.channels[&near], got.channels[&far]), (1, 1));
}

#[test]
fn only_lit_shadowed_mixed_lights_take_a_channel() {
    let mut scene = shadowmask();
    lamp(&mut scene, Vec3::ZERO, LightType::Point, LightMode::Baked);
    lamp(
        &mut scene,
        Vec3::ZERO,
        LightType::Point,
        LightMode::Realtime,
    );
    lamp(&mut scene, Vec3::ZERO, LightType::Area, LightMode::Mixed);
    let unshadowed = point(&mut scene, 0.0);
    let dark = point(&mut scene, 0.0);
    let nan = point(&mut scene, 0.0);
    let mut edit = |id: u32, f: &dyn Fn(&mut LightComponent)| {
        let mut l = scene.world.light(id).unwrap().clone();
        f(&mut l);
        scene.world.set_light(id, Some(l));
    };
    edit(unshadowed, &|l| l.cast_shadows = false);
    edit(dark, &|l| l.intensity = 0.0);
    edit(nan, &|l| l.range = f32::NAN);
    assert_eq!(assign_channels(&scene), Default::default());
}
