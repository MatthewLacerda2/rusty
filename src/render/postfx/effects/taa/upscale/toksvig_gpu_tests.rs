//! Toksvig's level takes TAAU's mip bias (#1702's review, item 3): at a render
//! scale of 0.5 a scene pixel covers twice the texels, and the normal map is read
//! a mip finer for it (`camera.lod.x`), so its spread must be counted at the level
//! read, not the scene pixel's (`forward::mapping::normal_level`). A busy map read
//! at its base level then widens nothing, and draws what its twin (the same x and
//! y, blue pinned at 255: no spread) draws. Without the bias it widens.

use glam::Vec3;

use super::gpu_tests::{scaled, upscales};
use crate::components::{LightComponent, MaterialAsset, MaterialComponent};
use crate::render::postfx::effects::taa::gpu_fixture::{eye, frame, renderer, taa_scene};
use crate::scene::authoring::{create_entity, default_light, Primitive};
use crate::scene::Scene;

/// A 224² busy normal map (blue its z, or 255 for the twin): about 0.7 texels a
/// native pixel on the wall below (`bias_gpu_tests`' 320² is one), so its
/// footprint reads half a level under the base, and a scene pixel at 0.5 half a
/// level over it.
fn busy_map(twin: bool) -> String {
    let mut state = 0x2545_f491u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        64 + (state >> 24) as u8 / 2
    };
    let img = image::RgbaImage::from_fn(224, 224, |_, _| {
        let (x, y) = (next(), next());
        let (nx, ny) = (x as f32 / 127.5 - 1.0, y as f32 / 127.5 - 1.0);
        let nz = ((1.0 - nx * nx - ny * ny).max(0.0).sqrt() * 127.5 + 127.5) as u8;
        image::Rgba([x, y, if twin { 255 } else { nz }, 255])
    });
    let path = crate::test_temp::dir().join(format!("rusty_1702_taau_bumps_{twin}.png"));
    img.save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// A glossy metal wall filling the view with `map`, a lamp before it; its
/// camera runs TAA when `taa`.
fn wall(taa: bool, map: String) -> Scene {
    let (mut scene, _) = taa_scene(taa, false);
    let id = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, 0.0, -7.0), Vec3::splat(10.0));
    drop(t);
    let m = Some(MaterialComponent::new("m".to_string()));
    scene.world.set_material(id, m);
    let asset = MaterialAsset {
        metallic: 1.0,
        roughness: 0.08,
        normal_map: Some(map.into()),
        ..MaterialAsset::default()
    };
    scene.materials.insert("m".to_string(), asset);
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 0.0, -1.0);
    let light = LightComponent {
        intensity: 20.0,
        cast_shadows: false,
        ..default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

/// The largest per-channel difference between two frames.
fn off(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap()
}

#[test]
fn gpu_an_upscaled_normal_map_counts_its_spread_at_the_biased_level() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let mut draw = |taa: bool, twin: bool| {
        let mut v = scaled(&r, 0.5);
        let f = frame(&mut r, &mut v, &wall(taa, busy_map(twin)), &cam);
        (f, upscales(&v))
    };
    // The first upscaled frame: the scene's own samples, before TAA averages them.
    let ((busy, up), (twin, _)) = (draw(true, false), draw(true, true));
    // Without TAA no bias: the scene pixel's level, over the base, which widens.
    let ((plain_busy, _), (plain_twin, _)) = (draw(false, false), draw(false, true));
    let (biased, unbiased) = (off(&busy, &twin), off(&plain_busy, &plain_twin));
    eprintln!("[taau] busy vs twin at 0.5: biased {biased}, unbiased {unbiased}");
    assert!(up, "0.5 upscales through TAA");
    assert!(unbiased > 8, "a level over the base widens: {unbiased}");
    assert!(
        biased <= 1,
        "read at its base level, it widens nothing: {biased}"
    );
}
