//! Directional lightmaps on the GPU (#810): a static, lightmapped floor whose normal
//! map tilts one half one way and the other half the other, lit only by a slanted
//! `Baked` sun, so every bit of its light comes from the lightmap.
//!
//! * **Directional bake:** the halves read the baked light through their own normals,
//!   so the half tilted toward the sun is visibly brighter than the other.
//! * **Non-directional bake** (`directional: false`): the lightmap ignores the normal
//!   map, so the two halves shade nearly alike, as before #810.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use glam::{Quat, Vec3};
use rusty::components::{LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent};
use rusty::dev::lightmap_bake::bake_scene_lightmaps;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::lighting::lightmap::BakeSettings;
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

/// A normal map whose left half (low U) tilts toward -tangent, right half +tangent.
fn bumps(path: &std::path::Path) {
    let img = image::RgbaImage::from_fn(64, 64, |x, _| match x < 32 {
        true => image::Rgba([64, 128, 218, 255]),
        false => image::Rgba([192, 128, 218, 255]),
    });
    img.save(path).expect("write normal map");
}

/// The bumpy static floor and a `Baked` sun slanting along the floor's tangent (X).
fn scene(dir: &std::path::Path) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    scene.skybox_path = String::new();
    let map = dir.join("bumps.png");
    bumps(&map);
    let floor = scene.add_entity("Floor".to_string());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(floor, true);
    scene.world.transform_mut(floor).unwrap().scale = Vec3::splat(0.25);
    let material = MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        roughness: 1.0,
        normal_map: Some(map.to_string_lossy().into_owned()),
        ..MaterialAsset::default()
    };
    scene.materials.insert("bumpy".to_string(), material);
    let bumpy = MaterialComponent {
        material: "bumpy".to_string(),
    };
    scene.world.set_material(floor, Some(bumpy));
    let sun = scene.add_entity("Sun".to_string());
    scene.world.set_light(
        sun,
        Some(LightComponent {
            light_type: LightType::Directional,
            color: Vec3::ONE,
            intensity: 2.0,
            range: 0.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            cast_shadows: false,
            mode: LightMode::Baked,
        }),
    );
    let down = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_z(0.8) * down;
    scene
}

/// The floor baked with `directional` and seen from above; `None` without an adapter.
fn shot(directional: bool) -> Option<image::RgbImage> {
    let name = format!("rusty_gpu_dir_lightmap_{directional}");
    let dir = std::env::temp_dir().join(&name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut scene = scene(&dir);
    let settings = BakeSettings {
        texels_per_unit: 8.0,
        samples: 16,
        bounces: 1,
        seed: 5,
        max_resolution: 32,
        filter_radius: 1,
        directional,
    };
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &settings),
        Ok(1)
    );
    assert_eq!(scene.lightmaps.directions.is_empty(), !directional);
    let camera = Camera::new(Vec3::new(0.0, 3.0, 0.0), 0.0, -89.9);
    let out = dir.join("shot.png");
    capture(&scene, &camera, &out, SIZE, SIZE)
        .expect("capture")
        .then_some(())?;
    Some(image::open(&out).unwrap().to_rgb8())
}

/// Mean brightness of the image's top and bottom thirds (the seam stays out): the
/// normal map's halves run along the floor's X, which the top-down view shows
/// vertically.
fn halves(img: &image::RgbImage) -> (f64, f64) {
    let band = |ys: std::ops::Range<u32>| {
        let px: Vec<u64> = ys
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .flat_map(|(x, y)| img.get_pixel(x, y).0)
            .map(u64::from)
            .collect();
        px.iter().sum::<u64>() as f64 / px.len() as f64
    };
    (band(0..SIZE / 3), band(SIZE - SIZE / 3..SIZE))
}

#[test]
fn normal_maps_reshape_a_directional_lightmap() {
    let Some(directional) = shot(true) else {
        eprintln!("[lightmap-dir] no GPU/software adapter — skipping");
        return;
    };
    let flat = shot(false).unwrap();
    let ((dl, dr), (fl, fr)) = (halves(&directional), halves(&flat));
    eprintln!("[lightmap-dir] directional {dl:.1}/{dr:.1}, flat {fl:.1}/{fr:.1}");
    assert!(
        fl + fr > 40.0,
        "the baked sun lights the floor: {fl:.1}/{fr:.1}"
    );
    let (bumps, flat) = ((dl - dr).abs(), (fl - fr).abs());
    assert!(flat < 4.0, "without directions the bumps vanish: {flat:.1}");
    assert!(bumps > flat + 15.0, "with them they show: {bumps:.1}");
}
