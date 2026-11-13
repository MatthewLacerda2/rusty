//! `highpass` keeps detail around mid-grey, drops a flat level, and wraps.

use super::*;
use crate::catalog::filter::test_probe::{assert_wraps, mean, relief, run};
use crate::recipe::OpKind;

fn highpass(radius: f32) -> OpKind {
    OpKind::Highpass(Highpass { radius })
}

#[test]
fn a_flat_image_reads_mid_grey() {
    let out = run(&highpass(0.05), &Image::filled(16, [0.8, 0.1, 0.3, 0.7]));
    for p in out.pixels() {
        assert!(p[..3].iter().all(|c| (c - 0.5).abs() < 1e-5), "{p:?}");
        assert_eq!(p[3], 0.7, "alpha kept");
    }
}

#[test]
fn detail_is_kept_centred_on_mid_grey() {
    let out = run(&highpass(0.2), &relief(64));
    assert!((mean(&out, 0) - 0.5).abs() < 1e-3);
    let spread = out
        .pixels()
        .iter()
        .map(|p| (p[0] - 0.5).abs())
        .fold(0.0, f32::max);
    assert!(spread > 0.05, "the sine pair's detail survives: {spread}");
}

#[test]
fn it_wraps() {
    assert_wraps(&highpass(0.05), &relief(32));
}
