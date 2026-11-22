use crate::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};
use rusty_asset::MapRef;
use rusty_components::RenderMode;

#[test]
fn gpu_a_cutout_caster_casts_no_shadow_where_its_texels_are_clear() {
    let albedo = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([255, 255, 255, 255 * (x / 2) as u8])
    });
    let path =
        rusty_test_support::temp::dir().join(format!("rusty_cutout_{}.png", std::process::id()));
    albedo.save(&path).expect("albedo written");
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (mut scene, slab) = slab_yard(None);
    let key = scene.ensure_material_key(slab).unwrap();
    let map = path.to_string_lossy().into_owned();
    scene.materials.get_mut(&key).unwrap().base_color_map = Some(MapRef::from(map));
    scene.materials.get_mut(&key).unwrap().alpha_cutoff = 0.5;

    let mut halves = |mode| {
        scene.materials.get_mut(&key).unwrap().render_mode = mode;
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
