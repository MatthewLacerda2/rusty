//! Atlas shadows clip as their casters do (#468, #648): under a spotlight, a slab
//! whose -X half is cut — a Cutout material's clear texels, or a dissolve burned
//! through — shadows only its +X half. Skips with no adapter.

use std::cell::RefCell;

use glam::{Quat, Vec3};

use crate::components::RenderMode;
use crate::render::gpu::pipelines::surface::cut_depth_tests::{dissolve, ground_halves, slab_yard};
use crate::render::gpu::pipelines::surface::mask_tests::Dissolve;
use crate::scene::authoring::{create_entity, material, Primitive};
use crate::scene::Scene;

/// [`slab_yard`] lit by a shadowing spotlight straight overhead instead of its sun.
fn under_a_spotlight(shader: Option<&str>) -> (Scene, u32) {
    let (mut scene, slab) = slab_yard(shader);
    for id in scene.world.ids_with_light() {
        scene.world.set_light(id, None);
    }
    scene.ambient_intensity = 0.0;
    let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::SpotLight));
    let mut t = scene.world.transform_mut(lamp).unwrap();
    (t.position, t.rotation) = (Vec3::Y * 8.0, Quat::from_rotation_x(-90f32.to_radians()));
    drop(t);
    let mut light = scene.world.light(lamp).unwrap().clone();
    (light.range, light.intensity, light.outer_cone) = (20.0, 200.0, 60.0);
    light.cast_shadows = true;
    scene.world.set_light(lamp, Some(light));
    (scene, slab)
}

#[test]
fn gpu_a_cutout_caster_clips_its_atlas_shadow() {
    let albedo = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([255, 255, 255, 255 * (x / 2) as u8])
    });
    let path = crate::test_temp::dir().join(format!("rusty_atlas_cut_{}.png", std::process::id()));
    albedo.save(&path).expect("albedo written");
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (mut scene, slab) = under_a_spotlight(None);
    let key = material::ensure_material_key(&mut scene, slab).unwrap();
    let map = path.to_string_lossy().into_owned();
    material::set_base_color_map(&mut scene.materials, &key, map);
    material::set_alpha_cutoff(&mut scene.materials, &key, 0.5);
    let mut halves = |mode| {
        material::set_render_mode(&mut scene.materials, &key, mode);
        ground_halves(&mut r, &scene)
    };
    let opaque = halves(RenderMode::Opaque);
    let [clear, solid] = halves(RenderMode::Cutout);
    let _ = std::fs::remove_file(&path);
    eprintln!("[atlas cutout] opaque {opaque:?}; cutout: clear {clear:.1}, solid {solid:.1}");
    assert!(
        (opaque[0] - opaque[1]).abs() < 5.0,
        "control: both halves shade"
    );
    assert!(
        (solid - opaque[1]).abs() < 5.0,
        "the opaque texels still shade"
    );
    assert!(clear > solid + 20.0, "the clear texels cast no shadow");
}

#[test]
fn gpu_a_dissolving_caster_clips_its_atlas_shadow() {
    let mask = image::RgbaImage::from_fn(4, 1, |x, _| image::Rgba([255 * (x / 2) as u8; 4]));
    let d = Dissolve::with_mask("atlas_half", &mask);
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(96, 96) else {
        return;
    };
    let (scene, slab) = under_a_spotlight(Some(&d.shader));
    let scene = RefCell::new(scene);
    dissolve(&scene, slab, &d, 0.0);
    let whole = ground_halves(&mut r, &scene.borrow());
    dissolve(&scene, slab, &d, 0.5);
    let [burned, kept] = ground_halves(&mut r, &scene.borrow());
    eprintln!("[atlas dissolve] whole {whole:?}; burned {burned:.1}, kept {kept:.1}");
    assert!(
        (whole[0] - whole[1]).abs() < 5.0,
        "control: both halves shade"
    );
    assert!((kept - whole[1]).abs() < 5.0, "the kept half still shades");
    assert!(burned > kept + 20.0, "the burned half's shadow is gone");
}
