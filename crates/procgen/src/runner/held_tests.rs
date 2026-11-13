//! Tests for what the runner holds between nodes (#2002): one-channel grayscale
//! and lazily decoded files change memory, never bytes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::json;

use super::held::Held;
use super::*;
use crate::ops::eval_node;
use crate::ops::test_graph::{bits, node};
use crate::source::{Loader, Source};

#[test]
fn only_a_lossless_gray_image_packs_to_one_channel() {
    let gray = Image::fill_uv(4, |u, _| [u, u, u, 1.0]);
    assert!(matches!(Held::pack(gray.clone()), Held::Gray(..)));
    assert_eq!(bits(&Held::pack(gray.clone()).into_image()), bits(&gray));
    // -0.0 beside 0.0, alpha below 1 and colour each keep all four channels.
    for p in [
        [-0.0, 0.0, 0.0, 1.0],
        [0.5, 0.5, 0.5, 0.9],
        [0.1, 0.2, 0.1, 1.0],
    ] {
        assert!(
            matches!(Held::pack(Image::filled(2, p)), Held::Rgba(_)),
            "{p:?}"
        );
    }
    let nan = Image::filled(2, [f32::NAN, f32::NAN, f32::NAN, 1.0]);
    assert_eq!(bits(&Held::pack(nan.clone()).view()), bits(&nan));
}

/// A graph of gray and colour nodes, each read more than once, with an `image`.
fn mixed() -> TextureRecipe {
    let nodes = vec![
        node(
            "n",
            json!({ "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 }),
            &[],
        ),
        node(
            "c",
            json!({ "op": "voronoi", "scale": 5, "output": "cells" }),
            &[],
        ),
        node(
            "i",
            json!({ "op": "image", "path": "a.png", "encoding": "linear" }),
            &[],
        ),
        node(
            "m",
            json!({ "op": "mix", "mode": "multiply" }),
            &["n", "c", "n"],
        ),
        node("k", json!({ "op": "mix", "mode": "add" }), &["m", "i", "n"]),
    ];
    TextureRecipe::new(16, nodes).with_seed(9).with_output("k")
}

fn file() -> Source {
    let texels = (0..64).map(|i| [i as f32 / 64.0, 0.5, 0.25, 1.0]).collect();
    Source::from_texels(8, 8, texels).unwrap()
}

/// `mixed` run node by node with nothing held between them: the bytes the runner
/// gave before it packed or loaded lazily.
fn reference(recipe: &TextureRecipe, sources: &Sources) -> Image {
    let mut done: Vec<(String, Image)> = Vec::new();
    for n in &recipe.nodes {
        let get = |id: &String| &done.iter().find(|(d, _)| d == id).unwrap().1;
        let inputs: Vec<&Image> = n.inputs.iter().map(get).collect();
        let seed = crate::hash::node_seed(recipe.seed, n.op.seed());
        let out = eval_node(&n.op, &inputs, recipe.extent(), seed, sources);
        done.push((n.id.clone(), out));
    }
    done.pop().unwrap().1
}

fn counting_loader(calls: Arc<AtomicUsize>) -> Loader {
    Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(file())
    })
}

#[test]
fn packing_and_lazy_loading_bake_the_bytes_an_unheld_run_does() {
    let mut eager = Sources::new();
    eager.insert("a.png", file());
    let want = bits(&reference(&mixed(), &eager));
    assert_eq!(bits(&evaluate_with(&mixed(), &eager).unwrap()), want);
    let calls = Arc::new(AtomicUsize::new(0));
    let lazy = Sources::lazy(["a.png".into()], counting_loader(calls.clone()));
    assert_eq!(bits(&evaluate_with(&mixed(), &lazy).unwrap()), want);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_file_no_requested_output_reads_is_never_decoded() {
    let calls = Arc::new(AtomicUsize::new(0));
    let lazy = Sources::lazy(["a.png".into()], counting_loader(calls.clone()));
    evaluate_with(&mixed().with_output("m"), &lazy).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // Two nodes reading one file decode it once.
    let mut recipe = mixed();
    let twin = json!({ "op": "image", "path": "a.png", "encoding": "srgb" });
    recipe.nodes.insert(3, node("j", twin, &[]));
    recipe.nodes.push(node(
        "out",
        json!({ "op": "mix", "mode": "add" }),
        &["k", "j"],
    ));
    evaluate_with(&recipe.with_output("out"), &lazy).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_file_the_loader_cannot_read_is_an_error_naming_the_node() {
    let loader: Loader = Arc::new(|_| Err("truncated".into()));
    let lazy = Sources::lazy(["a.png".into()], loader);
    let err = evaluate_with(&mixed(), &lazy).unwrap_err().to_string();
    assert_eq!(err, r#"node "i" (image): can't read "a.png": truncated"#);
}
