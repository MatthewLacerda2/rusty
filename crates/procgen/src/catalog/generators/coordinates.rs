//! The `coordinates` op: each pixel's own domain position (#2002).

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::recipe::{op, OpInfo, SOURCE};

/// Each pixel centre's position in the tile, `u` in red and `v` in green (blue 0,
/// alpha 1), Blender's *Texture Coordinate* node. Fed through `math` and into
/// `sample_at`, it remaps a map by any per-pixel function, a perspective or a polar
/// unwrap. It is the one ramp per axis, so it does not tile: `u` jumps from 1 to 0
/// at the seam.
pub struct Coordinates;

impl Row for Coordinates {
    const INFO: OpInfo = op("coordinates", "generator", SOURCE, &[]);
}

impl Op for Coordinates {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(|u, v| [u, v, 0.0, 1.0])
    }
}
