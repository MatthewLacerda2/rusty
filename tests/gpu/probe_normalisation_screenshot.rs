//! Probe normalisation by the baked diffuse (#1697). Two static polished metal floors
//! under one reflection probe whose cube is solid white: one lit by a `Baked` lamp,
//! one where no light reaches, so its lightmap is black. A metal has no diffuse term,
//! so each floor shows only the probe's reflection. Unnormalised both reflect the
//! white room alike; normalised, the dark one reflects at most a tenth of the lit one.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use crate::gpu::mean;
use crate::gpu::reflection_probe_per_object_screenshot::solid_cube_ktx2;
use glam::Vec3;
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::bake::lightmap::bake_scene_lightmaps;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::lightmap::BakeSettings;
use rusty::scene::{Camera, Scene, Sh9};

const SIZE: u32 = 32;
/// The dark floor's centre: well past the lamp's range.
const DARK_X: f32 = 20.0;

/// A uniform radiance-`l` SH: what the probe stores as its irradiance.
fn uniform_sh(l: f32) -> Sh9 {
    let mut sh = Sh9::zero();
    for dir in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
        sh.add_sample(dir, Vec3::splat(l), 4.0 * std::f32::consts::PI / 6.0);
    }
    sh
}

/// Both floors, the lamp over the lit one, and the probe over both, baked.
fn scene(name: &str, stored: Option<Sh9>) -> Scene {
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
    for x in [0.0, DARK_X] {
        let id = scene.add_entity(format!("Floor {x}"));
        let mesh = primitive_mesh_component(Primitive::Plane);
        scene.world.set_mesh(id, mesh);
        scene.world.set_static(id, true);
        let material = MaterialComponent::new("metal".to_string());
        scene.world.set_material(id, Some(material));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
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
    probe_and_bake(&mut scene, name, stored);
    scene
}

/// The white probe over both floors storing `stored`, then the lightmap bake.
fn probe_and_bake(scene: &mut Scene, name: &str, stored: Option<Sh9>) {
    let dir = crate::temp::dir().join(format!("rusty_gpu_1697_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cube = dir.join("white.ktx2");
    std::fs::write(&cube, solid_cube_ktx2(4, [1.0, 1.0, 1.0])).unwrap();
    let probes = &mut scene.reflection_probes;
    let i = probes.add(Vec3::new(DARK_X / 2.0, 1.0, 0.0), Vec3::ONE);
    probes.set_box(
        i,
        Vec3::new(-6.0, -2.0, -6.0),
        Vec3::new(DARK_X + 6.0, 6.0, 6.0),
    );
    probes.set_cubemap(i, cube.to_string_lossy().into_owned());
    probes.set_irradiance(i, stored);

    let settings = BakeSettings {
        texels_per_unit: 2.0,
        samples: 16,
        bounces: 1,
        seed: 3,
        max_resolution: 32,
        filter_radius: 1,
        directional: false,
    };
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    bake_scene_lightmaps(scene, Some(&path), &settings).unwrap();
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
fn a_floor_the_lightmap_holds_dark_reflects_at_most_a_tenth_of_the_lit_one() {
    let normalised = scene("normalised", Some(uniform_sh(0.05)));
    let Some(lit) = floor(&normalised, 0.0, "lit") else {
        eprintln!("[1697] no GPU/software adapter — skipping");
        return;
    };
    let dark = floor(&normalised, DARK_X, "dark").unwrap();
    let plain = scene("plain", None);
    let plain_dark = floor(&plain, DARK_X, "plain_dark").unwrap();
    eprintln!("[1697] lit={lit:.2} dark={dark:.2} unnormalised dark={plain_dark:.2}");
    assert!(
        lit > 60.0,
        "the lit floor reflects the white room: {lit:.2}"
    );
    assert!(
        dark <= lit * 0.1,
        "the dark floor reflects {dark:.2} of {lit:.2}"
    );
    assert!(
        plain_dark > 60.0,
        "a probe without stored irradiance is not normalised: {plain_dark:.2}"
    );
}
