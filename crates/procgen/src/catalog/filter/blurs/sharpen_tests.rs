//! `sharpen` steepens an edge, leaves flat ground alone, and wraps.

use super::*;
use crate::catalog::filter::test_probe::{assert_wraps, relief, run};
use crate::recipe::OpKind;

fn sharpen(intensity: f32) -> OpKind {
    OpKind::Sharpen(Sharpen {
        intensity,
        radius: 0.05,
    })
}

#[test]
fn flat_ground_is_unchanged() {
    let img = Image::filled(16, [0.25, 0.5, 0.75, 1.0]);
    assert_eq!(run(&sharpen(2.0), &img), img);
}

#[test]
fn an_edge_overshoots_on_both_sides() {
    // A vertical bar, dark on the left half.
    let img = Image::fill_uv(32, |u, _| [f32::from(u8::from(u >= 0.5)); 4]);
    let out = run(&sharpen(1.0), &img);
    let row = out.row(0);
    assert!(row[15][0] < 0.0 && row[16][0] > 1.0, "{:?}", &row[14..18]);
}

#[test]
fn zero_intensity_passes_through() {
    let img = relief(16);
    assert_eq!(run(&sharpen(0.0), &img), img);
}

#[test]
fn it_wraps() {
    assert_wraps(&sharpen(1.5), &relief(32));
}
