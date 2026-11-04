//! The `flood_bbox` op: each island's box, as a grey (Substance Designer's *Flood
//! Fill to BBox Size / Position*, #1741).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::flood;
use crate::procgen::recipe::{choice, op, Lit, OpInfo, OpParam, ONE};

/// Which of an island's box numbers [`FloodBbox`] writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BboxOutput {
    /// The box's longer side, in tiles (1 for an island that spans the tile).
    #[default]
    Size,
    /// The box's centre across, in `[0, 1)`.
    CenterX,
    /// The box's centre down, in `[0, 1)`.
    CenterY,
}

/// `flood_fill`'s islands → one flat grey per island from its bounding box, 0 off
/// them: its size or where it sits, to select, scale or offset per piece. A box
/// across a tile edge is measured as one box, its centre wrapped into the tile;
/// on an axis an island spans (a band across the tile) its centre reads 0.5.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FloodBbox {
    #[serde(default)]
    pub output: BboxOutput,
}

impl Row for FloodBbox {
    const INFO: OpInfo = op(
        "flood_bbox",
        "filter",
        ONE,
        &[OpParam {
            default: Some(Lit::Text("size")),
            ..choice("output", &["size", "center_x", "center_y"])
        }],
    );
}

impl Op for FloodBbox {
    fn run(&self, cx: &Cx) -> Image {
        let islands = flood::of_label_image(cx.input(0));
        let value = |i: &flood::Island| match self.output {
            BboxOutput::Size => i.size[0].max(i.size[1]),
            BboxOutput::CenterX => i.centre()[0],
            BboxOutput::CenterY => i.centre()[1],
        };
        flood::paint(&islands, value, |g, _, _| *g)
    }
}
