//! A surface cut reaches the depth passes (#648): a half-dissolved slab's shadow has
//! a hole under its burned half, and a crate dissolved away leaves no SSAO behind —
//! the floor reads as if it were never there. Skips with no adapter.

use std::cell::RefCell;

use glam::{Quat, Vec3};

use super::mask_tests::Dissolve;
use super::params_tests::lua;
use crate::core::quality::QualityPreset;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_visual_correction, material, Primitive};
use crate::scene::{Camera, Scene};

/// Fine enough for SSAO to resolve a crate's contact shadow.
const RES: u32 = 96;

/// A box at `position` scaled by `scale`; drawn by surface `shader` if any.
fn block(scene: &mut Scene, position: Vec3, scale: Vec3, shader: Option<&str>) -> u32 {
    let id = create_entity(scene, "Block", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    drop(t);
    if let Some(shader) = shader {
        let key = material::ensure_material_key(scene, id).unwrap();
        material::set_shader(&mut scene.materials, &key, shader.to_owned());
    }
    id
}

/// Ground under a high sun, and a 6 x 6 slab over it at y = 3 (drawn by `shader` if
/// any) whose -X half is u < 0.5. Returns the scene and the slab.
pub(crate) fn slab_yard(shader: Option<&str>) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.3);
    let ground = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(40.0, 1.0, 40.0));
    block(&mut scene, ground.0, ground.1, None);
    let slab = block(&mut scene, Vec3::Y * 3.0, Vec3::new(6.0, 0.2, 6.0), shader);
    (scene, slab)
}

/// Point `id`'s mask at `d`'s and set its `dissolve.amount`.
pub(crate) fn dissolve(scene: &RefCell<Scene>, id: u32, d: &Dissolve, amount: f32) {
    let script = format!(
        r#"Material.SetShaderTexture({id}, "mask", [==[{}]==])
        Material.SetShaderParam({id}, "dissolve.amount", {amount})"#,
        d.mask
    );
    lua(scene, &script);
}

/// The frame `cam` sees, as RGBA8.
fn frame(r: &mut Renderer, scene: &Scene, cam: &Camera) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES, 2);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, cam, &out, false);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, RES, RES)
}

/// Mean brightness of [`slab_yard`]'s ground under `x` and both slab halves at
/// `x = ∓1.5`, looked at from beneath the slab.
pub(crate) fn ground_halves(r: &mut Renderer, scene: &Scene) -> [f32; 2] {
    [-1.5, 1.5].map(|x| {
        // The sun leans the shadow ~0.8 units toward -Z; look where it lands.
        let cam = Camera::new(Vec3::new(x, 1.5, -0.8), -90.0, -89.0);
        let px = frame(r, scene, &cam);
        let rgb = px.chunks(4).flat_map(|p| &p[..3]);
        rgb.map(|&c| f32::from(c)).sum::<f32>() / (RES * RES * 3) as f32
    })
}

#[test]
fn gpu_a_half_dissolved_slab_casts_a_shadow_with_a_hole() {
    // Black (cut) where u < 0.5 — the slab's -X half, top and bottom — white beyond.
    let mask = image::RgbaImage::from_fn(4, 1, |x, _| image::Rgba([255 * (x / 2) as u8; 4]));
    let d = Dissolve::with_mask("half", &mask);
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, slab) = slab_yard(Some(&d.shader));
    let scene = RefCell::new(scene);

    dissolve(&scene, slab, &d, 0.0);
    let whole = ground_halves(&mut r, &scene.borrow());
    dissolve(&scene, slab, &d, 0.5);
    let [burned, kept] = ground_halves(&mut r, &scene.borrow());
    eprintln!("[cut shadow] whole {whole:?}; half-dissolved: burned {burned:.1}, kept {kept:.1}");
    assert!(
        (whole[0] - whole[1]).abs() < 5.0,
        "control: whole, both halves shade"
    );
    assert!((kept - whole[1]).abs() < 5.0, "the kept half still shades");
    assert!(burned > kept + 20.0, "the burned half's shadow is gone");
}

#[test]
fn gpu_a_dissolved_crate_leaves_no_ambient_occlusion() {
    let black = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255]));
    let d = Dissolve::with_mask("black", &black);
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    r.set_quality(QualityPreset::High);
    // A floor lit by ambient light alone, under an AO volume, with and without a crate.
    let yard = |with_crate: bool| {
        let mut scene = Scene::new();
        scene.skybox_path = String::new();
        scene.ambient_intensity = 0.6;
        let floor = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(20.0, 1.0, 20.0));
        block(&mut scene, floor.0, floor.1, None);
        let volume = create_entity(&mut scene, "Volume", None);
        let mut vc = default_visual_correction();
        (vc.bloom_active, vc.ssr_active, vc.ssao.active) = (false, false, true);
        scene.world.set_visual_correction(volume, Some(vc));
        let crate_id =
            with_crate.then(|| block(&mut scene, Vec3::Y, Vec3::splat(2.0), Some(&d.shader)));
        (RefCell::new(scene), crate_id)
    };
    let cam = Camera::new(Vec3::new(0.0, 2.5, 5.0), -90.0, -20.0);
    let (empty, _) = yard(false);
    let (scene, crate_id) = yard(true);
    let crate_id = crate_id.unwrap();
    let bare = frame(&mut r, &empty.borrow(), &cam);
    let differs = |a: &[u8]| {
        a.iter()
            .zip(&bare)
            .filter(|(x, y)| x.abs_diff(**y) > 6)
            .count()
    };

    dissolve(&scene, crate_id, &d, 0.0);
    let shown = differs(&frame(&mut r, &scene.borrow(), &cam));
    dissolve(&scene, crate_id, &d, 0.5);
    let left = differs(&frame(&mut r, &scene.borrow(), &cam));
    eprintln!(
        "[cut ssao] channels differing from the bare floor: standing {shown}, dissolved {left}"
    );
    assert!(shown > 60, "control: the crate shows");
    // Before #648 the prepass still held the crate: ~900 channels darkened around it.
    assert_eq!(left, 0, "the dissolved crate occludes nothing");
}
