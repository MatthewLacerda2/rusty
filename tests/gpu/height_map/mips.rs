//! A surface block's lookup under a height map reads as it does without one:
//! the `emissive_boost` block builds its coordinates with `coords_with`,
//! outside `parallax::displace`, so they are neither shifted nor bent (`bend` 0) and
//! are read at their own implicit derivatives, the mip a far surface needs. A far,
//! fine checker shows any other mip as a different frame.

use rusty::components::MaterialAsset;
use rusty::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use rusty::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

use super::{frame, grazing};
use crate::gpu::linear_data_maps_screenshot::{diff, SAME};

/// A surface variant of the one `emissive_boost` block; removed on drop.
struct Boost(String);

impl Boost {
    fn bake() -> Self {
        let name = format!("test_2191_boost_{}", std::process::id());
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: name.clone(),
            blocks: vec![BlockSel {
                id: "emissive_boost".into(),
                params: Default::default(),
            }],
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
        Self(name)
    }
}

impl Drop for Boost {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A 64-texel PNG: a 1-texel checker when `fine`, else all white.
fn png(name: &str, fine: bool) -> String {
    let path = crate::temp::dir().join(name);
    let img = image::RgbaImage::from_fn(64, 64, |x, y| match fine && (x + y) % 2 == 1 {
        true => image::Rgba([0, 0, 0, 255]),
        false => image::Rgba([255, 255, 255, 255]),
    });
    img.save(&path).expect("write png");
    path.to_string_lossy().into_owned()
}

/// A black, Opaque plane glowing only by the boost of a fine checker, 1024
/// repeats over 16 m; with a flat (all-white, so unshifted) height map at `scale`.
fn boosted(shader: &str, scale: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        roughness: 1.0,
        emissive: [0.0, 0.0, 0.0],
        emissive_map: Some(png("rusty_2191_fine.png", true).into()),
        tiling: [1024.0, 1024.0],
        shader: Some(shader.into()),
        height_map: Some(png("rusty_2191_level.png", false).into()),
        height_scale: scale,
        ..MaterialAsset::default()
    }
}

#[test]
fn a_surface_blocks_lookup_under_a_height_map_reads_its_mip() {
    let boost = Boost::bake();
    let cam = grazing(15.0);
    let mipped = frame(boosted(&boost.0, 0.0), &cam, "rusty_2191_boost_flat.png");
    let marched = frame(boosted(&boost.0, 0.02), &cam, "rusty_2191_boost_pom.png");
    let (Some(mipped), Some(marched)) = (mipped, marched) else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    let d = diff(&mipped, &marched);
    eprintln!("[height-map] the boost, far, under the height map: {d:.3}");
    // Measured: RTX 2060 0.000, lavapipe 0.000 (8.331 before `coords_with` filled
    // the derivatives).
    assert!(
        d < SAME,
        "the boost reads the mip it reads without the map: {d:.3}"
    );
}
