use glam::{Quat, Vec3};

use super::lightmap::bake_scene_lightmaps;
use super::probe::bake_scene_probes;
use crate::capture::CaptureHost;
use crate::screenshot::capture_into;
use rusty_asset::LightmapUvSettings;
use rusty_components::{LightComponent, LightMode, LightType};
use rusty_edit::asset_instance::{instantiate_asset, set_lightmap_uv_settings};
use rusty_scene::{Camera, Scene};
use rusty_test_support::models::quad_file as quad_model;

const SIZE: u32 = 32;

fn brightness(host: &mut CaptureHost, scene: &Scene, name: &str) -> Option<f64> {
    let path = rusty_test_support::temp::dir().join(format!("rusty_1540_{name}.png"));
    let camera = Camera::new(Vec3::new(0.0, 3.0, 0.0), 0.0, -89.0);
    if !capture_into(host, scene, &camera, &path, SIZE, SIZE).unwrap() {
        return None;
    }
    let image = image::open(&path).unwrap().to_rgb8();
    let sum: f64 = image.pixels().flat_map(|p| p.0).map(f64::from).sum();
    Some(sum / f64::from(3 * SIZE * SIZE))
}

fn floor_under_a_baked_sun() -> (Scene, String, String) {
    let model = quad_model("rusty_1540");
    let dir = std::path::Path::new(&model).parent().unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    let reference = format!("{model}::Quad");
    let floor = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    scene.world.set_static(floor, true);
    let mut t = scene.world.transform_mut(floor).unwrap();
    t.position = Vec3::new(-4.0, 0.0, 4.0);
    t.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    t.scale = Vec3::splat(8.0);
    drop(t);
    let sun = scene.add_entity("Sun".into());
    let light = LightComponent {
        light_type: LightType::Directional,
        intensity: 3.0,
        mode: LightMode::Baked,
        ..rusty_edit::authoring::default_light()
    };
    scene.world.set_light(sun, Some(light));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.2);
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_lightmap_size) = (2.0, 8, 1, 32);
    rusty_edit::save_to_file(&scene, &path).unwrap();
    (scene, model, path)
}

#[test]
fn gpu_a_bake_after_a_lightmap_uv_reimport_lights_the_open_world() {
    let _two = rusty_render::test_gpu::pair::headless_pair();
    let (mut scene, model, path) = floor_under_a_baked_sun();
    let mut editor = CaptureHost::new();
    let sun = scene.world.find_by_name("Sun").unwrap();
    scene.lightmaps.took_in_baked(sun);
    let Some(dark) = brightness(&mut editor, &scene, "before") else {
        eprintln!("[1540] no GPU/software adapter — skipping");
        return;
    };
    let on = LightmapUvSettings {
        generate: true,
        ..Default::default()
    };
    set_lightmap_uv_settings(&mut scene, &model, on).unwrap();
    let settings = scene.lighting_settings.lightmaps;
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &settings),
        Ok(1)
    );
    scene.probes.add_probe(Vec3::Y);
    assert_eq!(
        bake_scene_probes(&mut scene, &mut crate::bake::progress::BakeLog::off()),
        Ok(true)
    );
    let open = brightness(&mut editor, &scene, "open").unwrap();

    rusty_edit::save_to_file(&scene, &path).unwrap();
    let mut fresh = Scene::new();
    rusty_edit::load_from_file(&mut fresh, &path).unwrap();
    let loaded = brightness(&mut CaptureHost::new(), &fresh, "fresh").unwrap();
    eprintln!("[1540] before={dark:.1} open={open:.1} fresh load={loaded:.1}");
    assert!(
        loaded > dark + 30.0,
        "the lightmap lights the floor: {loaded:.1}"
    );
    assert!(
        (open - loaded).abs() < 1.0,
        "the open world draws as a fresh load does: {open:.1} vs {loaded:.1}"
    );
}
