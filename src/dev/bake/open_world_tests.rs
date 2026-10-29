//! A bake in an open world draws at once (#1540). The editor keeps one renderer
//! for the whole session while Generate Lighting's probe step draws the same scene
//! through a headless renderer of its own. A re-uploaded mesh used to be a flag the
//! first renderer cleared, so after a Generate Lightmap UVs re-import the bake's
//! renderer took the change and the editor's kept the floor's old vertices, with no
//! lightmap UV: every texel read one black corner of the atlas until Play/Stop
//! rebuilt the world. Skips when no adapter is present.

use glam::{Quat, Vec3};

use super::lightmap::bake_scene_lightmaps;
use super::probe::bake_scene_probes;
use crate::asset::LightmapUvSettings;
use crate::components::{LightComponent, LightMode, LightType};
use crate::dev::capture::CaptureHost;
use crate::dev::screenshot::capture_into;
use crate::scene::asset_instance::lightmap_test_support::quad_model;
use crate::scene::asset_instance::{instantiate_asset, set_lightmap_uv_settings};
use crate::scene::{Camera, Scene};

const SIZE: u32 = 32;

/// The mean brightness of `scene` seen from above through `host`; `None` without
/// an adapter.
fn brightness(host: &mut CaptureHost, scene: &Scene, name: &str) -> Option<f64> {
    let path = crate::test_temp::dir().join(format!("rusty_1540_{name}.png"));
    let camera = Camera::new(Vec3::new(0.0, 3.0, 0.0), 0.0, -89.0);
    if !capture_into(host, scene, &camera, &path, SIZE, SIZE).unwrap() {
        return None;
    }
    let image = image::open(&path).unwrap().to_rgb8();
    let sum: f64 = image.pixels().flat_map(|p| p.0).map(f64::from).sum();
    Some(sum / f64::from(3 * SIZE * SIZE))
}

/// A saved scene, no ambient: a static floor from a glTF quad with no lightmap UV,
/// under a `Baked` sun, so only its lightmap lights it. Returns it, the model's
/// path and the scene's.
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
    // The +Z quad over [0, 1]² turned to face up and spread over [-4, 4]².
    t.position = Vec3::new(-4.0, 0.0, 4.0);
    t.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    t.scale = Vec3::splat(8.0);
    drop(t);
    let sun = scene.add_entity("Sun".into());
    let light = LightComponent {
        light_type: LightType::Directional,
        intensity: 3.0,
        mode: LightMode::Baked,
        ..crate::scene::authoring::default_light()
    };
    scene.world.set_light(sun, Some(light));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.2);
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_resolution) = (2.0, 8, 1, 16);
    scene.save_to_file(&path).unwrap();
    (scene, model, path)
}

#[test]
fn gpu_a_bake_after_a_lightmap_uv_reimport_lights_the_open_world() {
    let (mut scene, model, path) = floor_under_a_baked_sun();
    let mut editor = CaptureHost::new();
    // The editor has drawn the floor as imported: no lightmap UV, no lightmap.
    let Some(dark) = brightness(&mut editor, &scene, "before") else {
        eprintln!("[1540] no GPU/software adapter — skipping");
        return;
    };
    // One script, as the game ran it: no frame between the re-import and the bake.
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
    // Generate Lighting's probe step, drawing the scene through its own renderer.
    scene.probes.add_probe(Vec3::Y);
    assert_eq!(bake_scene_probes(&mut scene), Ok(true));
    let open = brightness(&mut editor, &scene, "open").unwrap();

    scene.save_to_file(&path).unwrap();
    let mut fresh = Scene::new();
    fresh.load_from_file(&path).unwrap();
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
