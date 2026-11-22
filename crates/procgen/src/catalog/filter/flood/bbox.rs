use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::flood;
use crate::recipe::{choice, op, Lit, OpInfo, OpParam, ONE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BboxOutput {
    #[default]
    Size,
    CenterX,
    CenterY,
}

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
