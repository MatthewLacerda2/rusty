use glam::Vec3;

use super::tests::{quick, scene, scene_path};
use super::*;
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_edit::authoring::default_light;
use rusty_scene::lighting::MixedLightingMode;

fn lamp(scene: &mut Scene, name: &str, mode: LightMode, x: f32) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 1.0, 0.0);
    let light = LightComponent {
        light_type: LightType::Point,
        mode,
        cast_shadows: true,
        range: 4.0,
        ..default_light()
    };
    scene.world.set_light(id, Some(light));
    id
}

fn waits(scene: &Scene, id: u32) -> bool {
    scene.awaits_bake(id, &scene.world.light(id).unwrap().clone())
}

#[test]
fn a_mixed_lamp_made_baked_after_its_bake_waits_for_the_next() {
    let (mut scene, _, _) = scene();
    let mixed = lamp(&mut scene, "Mixed", LightMode::Mixed, 0.0);
    let baked = lamp(&mut scene, "Baked", LightMode::Baked, 1.0);
    let path = scene_path("covered_mixed");
    assert!(
        waits(&scene, baked) && !waits(&scene, mixed),
        "before any bake"
    );
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    assert!(scene.lights_awaiting_bake().is_empty(), "both taken in");
    scene.world.light_mut(mixed).unwrap().mode = LightMode::Baked;
    assert!(waits(&scene, mixed), "its direct light is in no bake");
    assert_eq!(scene.lights_awaiting_bake(), [mixed]);
    let light = scene.world.light(mixed).unwrap().clone();
    assert_eq!(
        scene.drawn_mode(mixed, &light, true),
        LightMode::Mixed,
        "previewed"
    );
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    assert!(scene.lights_awaiting_bake().is_empty(), "baked direct now");
    assert_eq!(scene.drawn_mode(mixed, &light, true), LightMode::Baked);
}

#[test]
fn a_bake_with_no_page_still_records_its_lights() {
    let mut scene = Scene::new();
    let crate_id = scene.add_entity("Crate".to_string());
    let mesh =
        rusty_edit::authoring::primitive_mesh_component(rusty_edit::authoring::Primitive::Box);
    scene.world.set_mesh(crate_id, mesh);
    let sun = lamp(&mut scene, "Lamp", LightMode::Baked, 0.0);
    assert_eq!(scene.lights_awaiting_bake(), [sun]);
    let path = scene_path("covered_no_page");
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &quick()),
        Ok(0)
    );
    assert!(
        scene.lightmaps.lights.is_empty(),
        "no page holds its bounce"
    );
    assert!(
        scene.lights_awaiting_bake().is_empty(),
        "the probes hold it"
    );
}

#[test]
fn a_fallen_back_mixed_lamp_is_not_previewed_twice() {
    let (mut scene, _, _) = scene();
    scene.lighting_settings.mixed_lighting_mode = MixedLightingMode::Shadowmask;
    let ids: Vec<u32> = (0..5)
        .map(|k| {
            lamp(
                &mut scene,
                &format!("Lamp {k}"),
                LightMode::Mixed,
                0.1 * k as f32,
            )
        })
        .collect();
    let path = scene_path("covered_fell_back");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let fell = ids[4];
    assert!(scene.lightmaps.fell_back.contains(&fell));
    let light = scene.world.light(fell).unwrap().clone();
    assert_eq!(scene.light_mode(fell, &light), LightMode::Baked);
    assert!(!scene.awaits_bake(fell, &light));
    assert_eq!(scene.drawn_mode(fell, &light, true), LightMode::Baked);
}
