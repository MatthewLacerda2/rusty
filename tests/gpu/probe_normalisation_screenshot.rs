//! Probe normalisation by the baked diffuse (#1697). Two static polished metal floors
//! on a white diffuse ground, under one reflection probe 1.2 m below a `Baked` lamp:
//! one floor under the lamp, one where no light reaches, so its lightmap is black.
//! The probe is baked for real (its stored irradiance is the capture's), then pointed
//! at a solid white cube so each floor's reflection is measurable. A metal has no
//! diffuse term, so each floor shows only the probe's reflection: the lit floor keeps
//! it whole (the lamp's direct light is not in the probe's stored irradiance, so
//! its nearness does not darken the floor), the dark one keeps at most a tenth.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use crate::gpu::mean;
use crate::gpu::reflection_probe_per_object_screenshot::solid_cube_ktx2;
use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::bake::lightmap::bake_scene_lightmaps;
use rusty::dev::bake::progress::BakeLog;
use rusty::dev::bake::reflection::bake_scene_reflections;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::settings::BakeSettings;
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 32;
/// The dark floor's centre: well past the lamp's range.
const DARK_X: f32 = 20.0;

/// Both floors, the lamp over the lit one, and the probe over both, baked.
fn scene(name: &str, normalised: bool) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let metal = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness: 0.1,
        ..MaterialAsset::default()
    };
    scene.materials.insert("metal".to_string(), metal);
    let white = MaterialAsset {
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert("white".to_string(), white);
    // A white ground and ceiling (a plane is 15 m across), and the two 2.25 m metal
    // floors on the ground: the probe's capture sees the room lit around the lamp.
    let middle = Vec3::new(DARK_X / 2.0, 0.0, 0.0);
    let (up, down) = (Quat::IDENTITY, Quat::from_rotation_x(std::f32::consts::PI));
    plane(&mut scene, "white", middle - Vec3::Y * 0.02, 3.0, up);
    plane(&mut scene, "white", middle + Vec3::Y * 3.0, 3.0, down);
    for x in [0.0, DARK_X] {
        plane(&mut scene, "metal", Vec3::X * x, 0.15, up);
    }
    let lamp = scene.add_entity("Lamp".to_string());
    let light = LightComponent {
        light_type: LightType::Point,
        intensity: 20.0,
        range: 6.0,
        mode: LightMode::Baked,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 1.5, 0.0);
    probe_and_bake(&mut scene, name, normalised);
    scene
}

/// A static plane of `material` at `at`, scaled by `scale` and turned by `rotation`.
fn plane(scene: &mut Scene, material: &str, at: Vec3, scale: f32, rotation: Quat) {
    let id = scene.add_entity(format!("{material} {at}"));
    let mesh = primitive_mesh_component(Primitive::Plane);
    scene.world.set_mesh(id, mesh);
    scene.world.set_static(id, true);
    let material = MaterialComponent::new(material.to_string());
    scene.world.set_material(id, Some(material));
    if let Some(mut t) = scene.world.transform_mut(id) {
        (t.position, t.scale, t.rotation) = (at, Vec3::splat(scale), rotation);
    }
}

/// The lightmap bake, then the probe 1.2 m under the lamp, baked; then pointed at a
/// white cube, keeping the irradiance its bake stored when `normalised`.
fn probe_and_bake(scene: &mut Scene, name: &str, normalised: bool) {
    let dir = crate::temp::dir().join(format!("rusty_gpu_1697_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cube = dir.join("white.ktx2");
    std::fs::write(&cube, solid_cube_ktx2(4, [0.3, 0.3, 0.3])).unwrap();
    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 16,
        bounces: 1,
        seed: 3,
        max_lightmap_size: 64,
        denoiser: rusty::scene::lighting::settings::Denoiser::Filter,
        filter_radius: 1,
        directional: false,
        backface_tolerance: BakeSettings::DEFAULT_BACKFACE_TOLERANCE,
        ..Default::default()
    };
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    bake_scene_lightmaps(scene, Some(&path), &settings).unwrap();
    let probes = &mut scene.reflection_probes;
    let i = probes.add(Vec3::new(0.0, 0.3, 0.0), Vec3::ONE);
    let corner = Vec3::new(-6.0, -2.0, -6.0);
    probes.set_box(i, corner, Vec3::new(DARK_X + 6.0, 6.0, 6.0));
    scene.lighting_settings.reflections.resolution = 32;
    bake_scene_reflections(scene, Some(&path), &mut BakeLog::off()).unwrap();
    let stored = scene.reflection_probes.probes[i].irradiance;
    let probes = &mut scene.reflection_probes;
    probes.set_cubemap(i, cube.to_string_lossy().into_owned());
    probes.set_irradiance(i, stored.filter(|_| normalised));
}

/// The mean of the floor at `x`, seen from above; `None` without an adapter.
fn floor(scene: &Scene, x: f32, name: &str) -> Option<f64> {
    let camera = Camera::new(Vec3::new(x, 1.0, 0.0), 0.0, -89.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_1697_{name}.png"));
    capture(scene, &camera, &path, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(mean(&image::open(&path).unwrap().to_rgb8()))
}

#[test]
fn a_lit_floor_keeps_its_reflection_and_a_dark_one_at_most_a_tenth() {
    let normalised = scene("normalised", true);
    let Some(lit) = floor(&normalised, 0.0, "lit") else {
        eprintln!("[1697] no GPU/software adapter — skipping");
        return;
    };
    assert!(normalised.reflection_probes.probes[0].irradiance.is_some());
    let dark = floor(&normalised, DARK_X, "dark").unwrap();
    let plain = scene("plain", false);
    let plain_lit = floor(&plain, 0.0, "plain_lit").unwrap();
    let plain_dark = floor(&plain, DARK_X, "plain_dark").unwrap();
    let (lit_share, dark_share) = (lit / plain_lit, dark / plain_dark);
    eprintln!("[1697] display value kept: lit {lit_share:.3}, dark {dark_share:.3}");
    assert!(
        plain_lit > 60.0 && plain_dark > 60.0,
        "both reflect the cube"
    );
    assert!(lit >= plain_lit * 0.95, "the lit floor keeps it: {lit:.2}");
    assert!(dark <= plain_dark * 0.1, "the dark floor: {dark:.2}");
}
