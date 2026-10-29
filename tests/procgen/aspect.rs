//! Non-square bakes (#1493): a `size = [w, h]` recipe samples the same unit tile
//! with more pixels on one axis, tiles on both, and a square `size` is exactly the
//! `resolution` it spells out.

use rusty::procgen::bake::Slot;
use rusty::procgen::recipe::{
    Blur, BlurKind, Brick, BrickOutput, BumpToNormal, Cavity, Curvature, Distance, Node, OpKind,
    TextureRecipe, WhiteNoise,
};
use rusty::procgen::{bake_recipe, evaluate, Image, Size, Sources};

use super::node;
use super::seams::{generators, seam_and_interior};
use crate::temp::path as tmp;

/// Every generator, plus a brick height through each op whose reach is in pixels.
fn recipes() -> Vec<(String, Vec<Node>)> {
    let mut all: Vec<_> = generators()
        .into_iter()
        .map(|(label, op)| (label, vec![node("g", op, &[])]))
        .collect();
    let brick = OpKind::Brick(Brick {
        rows: 4.0,
        cols: 16.0,
        mortar: 0.1,
        output: BrickOutput::Mask,
        seed: 0,
    });
    let after = [
        OpKind::Blur(Blur {
            radius: 0.02.into(),
            kind: BlurKind::Gaussian,
        }),
        OpKind::Cavity(Cavity { radius: 0.03 }),
        OpKind::Curvature(Curvature {
            radius: 0.02,
            strength: 2.0.into(),
        }),
        OpKind::Distance(Distance {
            radius: 0.05.into(),
        }),
        OpKind::BumpToNormal(BumpToNormal {
            strength: 0.5.into(),
        }),
    ];
    for op in after {
        let label = format!("{op:?}");
        all.push((
            label,
            vec![node("h", brick.clone(), &[]), node("o", op, &["h"])],
        ));
    }
    let noise = OpKind::WhiteNoise(WhiteNoise::default());
    all.push(("white noise".into(), vec![node("w", noise, &[])]));
    all
}

/// [`super::seams::assert_seamless`] with 10% slack: on the 64 rows down, the
/// wrap is one neighbour pair in 64, and a periodic field's steepest pair lands on
/// it by chance (turbulence at scale 1 overshoots by 0.7%). A real seam is a jump
/// many times the interior's.
fn assert_tiles(label: &str, img: &Image) {
    for horizontal in [true, false] {
        let (seam, interior) = seam_and_interior(img, horizontal);
        let bound = interior * 1.1 + 1e-4;
        assert!(
            seam <= bound,
            "{label}: seam {seam} > {bound} ({horizontal})"
        );
    }
}

fn eval(recipe: TextureRecipe) -> Image {
    evaluate(&recipe.with_seed(5)).expect("recipe evaluates")
}

#[test]
fn a_256_by_64_recipe_bakes_a_256_by_64_png_that_tiles_on_both_axes() {
    for (label, nodes) in recipes() {
        let img = eval(TextureRecipe::new(0, nodes).with_size([256, 64]));
        assert_eq!(img.size(), Size::new(256, 64), "{label}");
        // White noise has no neighbour to bound a seam by: its jumps are all noise.
        if label != "white noise" {
            assert_tiles(&label, &img);
        }
    }
    let (_, nodes) = recipes().remove(0);
    let path = tmp("rusty_procgen_trim_sheet.png");
    let recipe = TextureRecipe::new(0, nodes).with_size([256, 64]);
    bake_recipe(&recipe, &Sources::new(), Slot::BaseColor, &path).expect("bakes");
    let png = image::open(&path).expect("the PNG loads back");
    assert_eq!((png.width(), png.height()), (256, 64));
    std::fs::remove_file(path).ok();
}

#[test]
fn a_square_size_bakes_exactly_as_its_resolution() {
    for n in [32, 128] {
        for (label, nodes) in recipes() {
            let by_res = eval(TextureRecipe::new(n, nodes.clone()));
            let by_size = eval(TextureRecipe::new(0, nodes).with_size([n, n]));
            assert!(by_res == by_size, "{label} at {n}");
        }
    }
}

#[test]
fn a_size_recipe_needs_no_resolution_and_round_trips_without_one() {
    let json = r#"{ "size": [64, 16], "nodes": [{ "id": "c", "op": "constant", "color": [1, 0, 0, 1] }] }"#;
    let recipe: TextureRecipe = serde_json::from_str(json).expect("parses");
    let back = recipe.to_json().expect("serializes");
    assert!(!back.contains("resolution"), "{back}");
    assert_eq!(
        serde_json::from_str::<TextureRecipe>(&back).unwrap(),
        recipe
    );
}
