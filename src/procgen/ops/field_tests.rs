//! `Field::at_pixel` (#1566): a generator's `(u, v)` lands on the pixel it falls in,
//! on an image wider than it is tall, and the far edge (`u` or `v` of 1) clamps to
//! the last pixel rather than reading past the row.

use super::*;
use crate::procgen::image_buf::Size;

/// 4 across, 2 down, each pixel's red its row-major index.
fn indexed() -> Image {
    let mut img = Image::new(Size::new(4, 2));
    img.map_indexed(|i, _| [i as f32, 0.0, 0.0, 1.0]);
    img
}

#[test]
fn at_pixel_reads_the_pixel_a_point_falls_in() {
    let img = indexed();
    let field = Field::Map {
        img: &img,
        min: 0.0,
        max: 1.0,
    };
    for i in 0..8 {
        let (x, y) = ((i % 4) as f32, (i / 4) as f32);
        let (u, v) = ((x + 0.5) / 4.0, (y + 0.5) / 2.0);
        assert_eq!(field.at_pixel(u, v), i as f32, "pixel ({x}, {y})");
    }
}

#[test]
fn at_pixel_clamps_the_far_edge_to_the_last_pixel() {
    let img = indexed();
    let field = Field::Map {
        img: &img,
        min: 0.0,
        max: 1.0,
    };
    assert_eq!(field.at_pixel(1.0, 0.0), 3.0, "u = 1 is the last column");
    assert_eq!(field.at_pixel(0.0, 1.0), 4.0, "v = 1 is the last row");
    assert_eq!(field.at_pixel(1.0, 1.0), 7.0);
}

#[test]
fn at_pixel_stretches_red_to_the_link_range() {
    let img = Image::filled(Size::new(4, 2), [0.25, 0.0, 0.0, 1.0]);
    let field = Field::Map {
        img: &img,
        min: 2.0,
        max: 6.0,
    };
    assert_eq!(field.at_pixel(0.6, 0.6), 3.0);
}
