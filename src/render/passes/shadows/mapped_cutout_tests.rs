//! A Cutout caster's shadow holes sit where its drawn holes do under its material's
//! Tiling/Offset and world mapping (#1594): the clip read the albedo at the raw UV0,
//! so a flipped or triplanar-projected fence cast the holes of its untiled image.
//! [`slab_yard`]'s slab is clear where its UV0 `u < 0.5` (its -X half); each mapping
//! below moves the clear texels to its +X half, so the shadow must follow. Skips with
//! no adapter.

use crate::components::{MaterialAsset, RenderMode, UvMapping};
use crate::render::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};
use crate::scene::authoring::material;

/// `[-X, +X]` ground brightness under the Cutout slab, mapped by `map`.
fn halves_mapped(name: &str, map: impl Fn(&mut MaterialAsset)) -> Option<[f32; 2]> {
    // Clear where u < 0.5, opaque white beyond.
    let albedo = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([255, 255, 255, 255 * (x / 2) as u8])
    });
    let path = crate::test_temp::dir().join(format!("rusty_{name}_{}.png", std::process::id()));
    albedo.save(&path).expect("albedo written");
    let mut r = crate::render::test_gpu::headless_or_skip(32, 32)?;
    let (mut scene, slab) = slab_yard(None);
    let key = material::ensure_material_key(&mut scene, slab).unwrap();
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
    // u' = 1 - u: the clear texels move to the +X half.
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
    // The slab's faces read the world-XZ projection at a 12 m repeat: x in
    // [-3, 0) wraps to u in [0.75, 1) (opaque), x in [0, 3) to [0, 0.25) (clear).
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
