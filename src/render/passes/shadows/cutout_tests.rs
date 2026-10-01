//! A Cutout caster's shadow is clipped as its surface is (#242, #648): the depth pass
//! had no fragment stage, so a leaf card or a grate cast a solid rectangle. A slab
//! whose albedo is clear on its -X half shadows only its +X half once it is Cutout;
//! Opaque, it shadows both (the control). Skips with no adapter.

use crate::components::RenderMode;
use crate::render::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};
use crate::scene::authoring::material;

#[test]
fn gpu_a_cutout_caster_casts_no_shadow_where_its_texels_are_clear() {
    // Clear where u < 0.5 — the slab's -X half — opaque white beyond.
    let albedo = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([255, 255, 255, 255 * (x / 2) as u8])
    });
    let path = crate::test_temp::dir().join(format!("rusty_cutout_{}.png", std::process::id()));
    albedo.save(&path).expect("albedo written");
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (mut scene, slab) = slab_yard(None);
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
    eprintln!("[cutout shadow] opaque {opaque:?}; cutout: clear {clear:.1}, solid {solid:.1}");
    assert!(
        (opaque[0] - opaque[1]).abs() < 5.0,
        "control: opaque shades both halves"
    );
    assert!(
        (solid - opaque[1]).abs() < 5.0,
        "the opaque texels still shade"
    );
    assert!(clear > solid + 20.0, "the clear texels cast no shadow");
}
