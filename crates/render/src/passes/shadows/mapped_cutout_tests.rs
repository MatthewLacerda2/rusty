use crate::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};
use rusty_components::{MaterialAsset, RenderMode, UvMapping};

fn halves_mapped(name: &str, map: impl Fn(&mut MaterialAsset)) -> Option<[f32; 2]> {
    let albedo = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([255, 255, 255, 255 * (x / 2) as u8])
    });
    let path =
        rusty_test_support::temp::dir().join(format!("rusty_{name}_{}.png", std::process::id()));
    albedo.save(&path).expect("albedo written");
    let mut r = crate::test_gpu::headless_or_skip(32, 32)?;
    let (mut scene, slab) = slab_yard(None);
    let key = scene.ensure_material_key(slab).unwrap();
    let asset = scene.materials.get_mut(&key).unwrap();
    asset.base_color_map = Some(path.to_string_lossy().into_owned().into());
    (asset.alpha_cutoff, asset.render_mode) = (0.5, RenderMode::Cutout);
    map(asset);
    let halves = ground_halves(&mut r, &scene);
    let _ = std::fs::remove_file(&path);
    eprintln!("[{name}] -X {:.1}, +X {:.1}", halves[0], halves[1]);
    Some(halves)
}

#[test]
fn gpu_a_tiled_cutouts_shadow_follows_its_tiling_and_offset() {
    let Some([solid, clear]) = halves_mapped("flipped", |m| {
        (m.tiling, m.offset) = ([-1.0, 1.0], [1.0, 0.0]);
    }) else {
        return;
    };
    assert!(
        clear > solid + 20.0,
        "the shadow's hole moved with the texels"
    );
}

#[test]
fn gpu_a_triplanar_cutouts_shadow_follows_its_projection() {
    let Some([solid, clear]) = halves_mapped("triplanar", |m| {
        (m.uv_mapping, m.world_size) = (UvMapping::Triplanar, 12.0);
    }) else {
        return;
    };
    assert!(
        clear > solid + 20.0,
        "the shadow's hole follows the projection"
    );
}
