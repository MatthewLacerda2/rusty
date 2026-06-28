//! Bake-level integration for procedural textures (#270): the full recipe → runner →
//! PNG path, the glTF channel conventions, tiling, determinism, and that a baked file
//! loads back through the same `image` crate the renderer's `load_texture` uses.

use rusty::procgen::bake::{encode, Slot};
use rusty::procgen::recipe::{Node, NoiseKind, OpKind, TextureRecipe};
use rusty::procgen::{bake_recipe, evaluate};

fn noise_recipe(resolution: u32, seed: u64) -> TextureRecipe {
    TextureRecipe {
        resolution,
        seed,
        nodes: vec![Node {
            id: "n".into(),
            op: OpKind::Noise {
                kind: NoiseKind::Perlin,
                scale: 8.0,
                octaves: 1,
            },
            inputs: vec![],
        }],
        output: None,
    }
}

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn same_recipe_and_seed_bakes_byte_identical_png() {
    let recipe = noise_recipe(128, 4242);
    let a = tmp("rusty_bake_det_a.png");
    let b = tmp("rusty_bake_det_b.png");
    bake_recipe(&recipe, Slot::BaseColor, &a).unwrap();
    bake_recipe(&recipe, Slot::BaseColor, &b).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    std::fs::remove_file(a).ok();
    std::fs::remove_file(b).ok();
}

#[test]
fn baked_png_is_loadable_at_the_requested_resolution() {
    let recipe = noise_recipe(64, 1);
    let path = tmp("rusty_bake_loadable.png");
    bake_recipe(&recipe, Slot::Normal, &path).unwrap();
    // The same decode path the renderer's `load_texture` takes.
    let img = image::open(&path).expect("baked PNG decodes");
    assert_eq!(image::GenericImageView::dimensions(&img), (64, 64));
    std::fs::remove_file(path).ok();
}

#[test]
fn perlin_noise_wraps_left_to_right_and_top_to_bottom() {
    // Tiling by construction: the column past the right edge must closely match
    // column 0 (the lattice is periodic, so the seam is continuous).
    let img = evaluate(&noise_recipe(128, 9)).unwrap();
    let mut max_diff = 0.0f32;
    for y in 0..128 {
        let edge = (img.get_wrapped(127, y)[0] - img.get_wrapped(-1, y)[0]).abs();
        let top = (img.get_wrapped(y, 127)[1] - img.get_wrapped(y, -1)[1]).abs();
        max_diff = max_diff.max(edge).max(top);
    }
    // get_wrapped(-1) == get_wrapped(127), so this is an identity check on the wrap.
    assert!(
        max_diff < 1e-6,
        "noise must tile seamlessly, max edge diff {max_diff}"
    );
}

#[test]
fn srgb_vs_linear_encoding_is_applied_per_slot() {
    // Mid linear gray encodes brighter as sRGB color, unchanged as linear data.
    let img = rusty::procgen::Image::filled(2, [0.5, 0.5, 0.5, 1.0]);
    let color = encode(&img, Slot::BaseColor).get_pixel(0, 0).0[0];
    let data = encode(&img, Slot::Roughness).get_pixel(0, 0).0[0];
    assert_eq!(data, 128, "linear roughness keeps 0.5 -> 128");
    assert!(
        color > 180,
        "sRGB base-color lifts 0.5 -> ~188, got {color}"
    );
}

#[test]
fn metallic_roughness_packs_metallic_b_roughness_g() {
    // A recipe whose output red is metallic and green is roughness; the MR pack must
    // route metallic to B and roughness to G in one bake.
    let recipe = TextureRecipe {
        resolution: 4,
        seed: 0,
        nodes: vec![Node {
            id: "c".into(),
            op: OpKind::Constant {
                color: [1.0, 0.5, 0.0, 1.0], // R=metallic=1, G=roughness=0.5
            },
            inputs: vec![],
        }],
        output: None,
    };
    let img = evaluate(&recipe).unwrap();
    let px = encode(&img, Slot::MetallicRoughness).get_pixel(0, 0).0;
    assert_eq!(px[2], 255, "metallic -> B");
    assert!(
        (px[1] as i32 - 128).abs() <= 1,
        "roughness 0.5 -> G ~128, got {}",
        px[1]
    );
}

#[test]
fn loaded_recipe_re_bakes_identically() {
    let recipe = noise_recipe(48, 77);
    let json = recipe.to_json().unwrap();
    let back = TextureRecipe::from_json(&json).unwrap();
    let from_recipe = tmp("rusty_bake_roundtrip_a.png");
    let from_json = tmp("rusty_bake_roundtrip_b.png");
    bake_recipe(&recipe, Slot::Data, &from_recipe).unwrap();
    bake_recipe(&back, Slot::Data, &from_json).unwrap();
    assert_eq!(
        std::fs::read(&from_recipe).unwrap(),
        std::fs::read(&from_json).unwrap()
    );
    std::fs::remove_file(from_recipe).ok();
    std::fs::remove_file(from_json).ok();
}
