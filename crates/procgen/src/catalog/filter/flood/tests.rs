//! The flood-fill ops on small masks: one flat value per island, an island across
//! the edge read as one, boxes that wrap, and readers that never panic on what
//! isn't a label image.

use super::bbox::{BboxOutput, FloodBbox};
use super::fill::FloodFill;
use super::gradient::FloodGradient;
use super::random::FloodRandom;
use crate::catalog::filter::test_probe::{assert_close, rolled, run};
use crate::hash;
use crate::image_buf::Image;
use crate::ops::eval_node;
use crate::recipe::OpKind;
use crate::source::Sources;

const FILL: OpKind = OpKind::FloodFill(FloodFill { threshold: 0.5 });

/// Squares on a 32² tile: one in the middle, one across the left/right edge, one
/// across the corner.
fn squares() -> Image {
    Image::fill_uv(32, |u, v| {
        let box_ = |a: f32, b: f32, x: f32| (a..b).contains(&x);
        let mid = box_(0.3, 0.5, u) && box_(0.3, 0.6, v);
        let edge = !(0.1..=0.9).contains(&u) && box_(0.6, 0.8, v);
        let corner = !(0.05..=0.85).contains(&u) && !(0.1..=0.9).contains(&v);
        [f32::from(u8::from(mid || edge || corner)), 0.0, 0.0, 1.0]
    })
}

/// `reader` on `flood_fill`'s output, its own seed folded in as the runner does.
fn after_fill(reader: &OpKind, mask: &Image) -> Image {
    let labels = run(&FILL, mask);
    let seed = hash::node_seed(0, reader.seed());
    eval_node(reader, &[&labels], labels.size(), seed, &Sources::new())
}

fn random(seed: u32) -> OpKind {
    OpKind::FloodRandom(FloodRandom { seed })
}

/// Pixel `(x, y)`'s red.
fn at(img: &Image, x: i64, y: i64) -> f32 {
    img.get_wrapped(x, y)[0]
}

#[test]
fn each_island_reads_one_flat_value_of_its_own() {
    let out = after_fill(&random(3), &squares());
    let (mid, edge, corner) = (at(&out, 12, 12), at(&out, 31, 22), at(&out, 31, 31));
    assert_eq!(at(&out, 15, 18), mid);
    assert_eq!(at(&out, 0, 22), edge, "one value on both sides of the edge");
    assert_eq!(at(&out, 0, 0), corner, "one value at all four corners");
    assert!(mid != edge && edge != corner && mid != corner);
    assert_eq!(at(&out, 20, 2), 0.0, "off the islands");
    assert_eq!(out, after_fill(&random(3), &squares()), "deterministic");
    assert_ne!(out, after_fill(&random(4), &squares()), "the seed moves it");
}

#[test]
fn a_value_comes_from_the_island_not_its_scan_number() {
    // A speck in the top row is scanned first, renumbering every other island.
    let mut more = squares();
    more.pixels_mut()[20] = [1.0, 0.0, 0.0, 1.0];
    let (a, b) = (
        after_fill(&random(1), &squares()),
        after_fill(&random(1), &more),
    );
    for (x, y) in [(12, 12), (31, 22), (0, 0)] {
        assert_eq!(at(&a, x, y), at(&b, x, y), "({x}, {y})");
    }
}

#[test]
fn boxes_and_gradients_move_with_a_rolled_mask() {
    let gradient = OpKind::FloodGradient(FloodGradient {
        angle: 0.1,
        angle_jitter: 0.0,
        seed: 0,
    });
    let size = OpKind::FloodBbox(FloodBbox::default());
    for op in [gradient, size] {
        let (a, b) = (
            after_fill(&op, &rolled(&squares(), 5, 3)),
            after_fill(&op, &squares()),
        );
        assert_close(&a, &rolled(&b, 5, 3), 1e-5, op.tag());
    }
}

#[test]
fn a_box_across_the_edge_is_one_box() {
    let bbox = |output| after_fill(&OpKind::FloodBbox(FloodBbox { output }), &squares());
    let centre = bbox(BboxOutput::CenterX);
    let x = at(&centre, 0, 22);
    assert!(!(0.01..=0.99).contains(&x), "centre wraps to the edge: {x}");
    assert_eq!(at(&centre, 31, 22), x);
    let size = bbox(BboxOutput::Size);
    assert_eq!(
        at(&size, 0, 22),
        0.21875,
        "its longer side: 7 of 32 pixels down"
    );
    assert_eq!(at(&bbox(BboxOutput::CenterY), 12, 12), 0.453_125);
}

#[test]
fn a_gradient_ramps_across_each_box() {
    let flat = OpKind::FloodGradient(FloodGradient::default());
    let out = after_fill(&flat, &squares());
    let row: Vec<f32> = (10..16).map(|x| at(&out, x, 12)).collect();
    assert!(row.windows(2).all(|p| p[0] < p[1]), "{row:?}");
    assert!(row[0] > 0.0 && row[5] < 1.0, "{row:?}");
    // Across the edge: x = 29..31 then 0..2 rise in turn.
    let edge: Vec<f32> = (29..35).map(|x| at(&out, x, 22)).collect();
    assert!(edge.windows(2).all(|p| p[0] < p[1]), "{edge:?}");
}

#[test]
fn a_reader_fed_an_ordinary_image_reads_it_as_one_island() {
    let opaque = Image::filled(16, [0.3, 0.2, 0.1, 1.0]);
    let out = run(&random(2), &opaque);
    let first = out.pixels()[0];
    assert!(out.pixels().iter().all(|p| *p == first));
    let clear = Image::filled(16, [0.3, 0.2, 0.1, 0.0]);
    assert!(run(&random(2), &clear).pixels().iter().all(|p| p[0] == 0.0));
}

#[test]
fn edge_masks_neither_panic_nor_read_nan() {
    let (on, off) = ([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
    let nan = [f32::NAN, 0.0, 0.0, 1.0];
    let jitter = OpKind::FloodGradient(FloodGradient {
        angle: f32::NAN,
        angle_jitter: 1.0,
        seed: 1,
    });
    let readers = [random(1), jitter, OpKind::FloodBbox(FloodBbox::default())];
    for fill in [on, off, nan] {
        let labels = run(&FILL, &Image::filled(8, fill));
        for reader in &readers {
            let out = run(reader, &labels);
            assert!(out.pixels().iter().flatten().all(|c| c.is_finite()));
        }
    }
}
