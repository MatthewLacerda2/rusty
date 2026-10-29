//! `bright_contrast` and `hue_sat_value` against values worked by hand (#1566), with
//! numbers and with links read per pixel on an image wider than it is tall.

use super::*;
use crate::procgen::image_buf::Size;

/// 3 across, 2 down.
const SIZE: Size = Size::new(3, 2);

/// A link whose red at pixel `i` is `i / 10`, read over `[0, 1]`.
fn ramp_by_index() -> Image {
    let mut img = Image::new(SIZE);
    img.map_indexed(|i, _| [i as f32 / 10.0, 0.0, 0.0, 1.0]);
    img
}

fn link(img: &Image) -> Field<'_> {
    Field::Map {
        img,
        min: 0.0,
        max: 1.0,
    }
}

fn close(got: Rgba, want: [f32; 3], what: &str) {
    for c in 0..3 {
        assert!(
            (got[c] - want[c]).abs() < 1e-5,
            "{what}: {got:?} vs {want:?}"
        );
    }
}

#[test]
fn bright_contrast_offsets_then_scales_about_mid_gray() {
    let img = Image::filled(SIZE, [0.8, 0.3, 0.6, 0.4]);
    let out = bright_contrast(img, Field::Const(0.1), Field::Const(2.0));
    // (c + 0.1 − 0.5) × 2 + 0.5
    close(out.pixels()[0], [1.3, 0.3, 0.9], "numbers");
    assert_eq!(out.pixels()[0][3], 0.4, "alpha passes through");
}

#[test]
fn bright_contrast_reads_a_linked_brightness_at_each_pixel() {
    let bright = ramp_by_index();
    let img = Image::filled(SIZE, [0.5, 0.5, 0.5, 1.0]);
    let out = bright_contrast(img, link(&bright), Field::Const(2.0));
    for (i, p) in out.pixels().iter().enumerate() {
        let v = 0.2 * i as f32 + 0.5;
        close(*p, [v, v, v], &format!("pixel {i}"));
    }
}

#[test]
fn hue_sat_value_rotates_scales_and_scales() {
    let red = || Image::filled(SIZE, [1.0, 0.0, 0.0, 0.7]);
    let (one, none) = (Field::Const(1.0), Field::Const(0.0));
    let hue = hue_sat_value(red(), Field::Const(1.0 / 3.0), one, one);
    close(
        hue.pixels()[0],
        [0.0, 1.0, 0.0],
        "a third of a turn is green",
    );
    let sat = hue_sat_value(red(), none, Field::Const(0.5), one);
    close(sat.pixels()[0], [1.0, 0.5, 0.5], "half saturation");
    let value = hue_sat_value(red(), none, one, Field::Const(0.5));
    close(value.pixels()[0], [0.5, 0.0, 0.0], "half value");
    assert_eq!(value.pixels()[0][3], 0.7, "alpha passes through");
}

#[test]
fn hue_sat_value_reads_a_linked_value_at_each_pixel() {
    let value = ramp_by_index();
    let img = Image::filled(SIZE, [1.0, 0.0, 0.0, 1.0]);
    let out = hue_sat_value(img, Field::Const(0.0), Field::Const(1.0), link(&value));
    for (i, p) in out.pixels().iter().enumerate() {
        close(*p, [i as f32 / 10.0, 0.0, 0.0], &format!("pixel {i}"));
    }
}
