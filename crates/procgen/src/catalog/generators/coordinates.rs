use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::recipe::{op, OpInfo, SOURCE};

pub struct Coordinates;

impl Row for Coordinates {
    const INFO: OpInfo = op("coordinates", "generator", SOURCE, &[]);
}

impl Op for Coordinates {
    fn run(&self, cx: &Cx) -> Image {
        cx.paint(|u, v| [u, v, 0.0, 1.0])
    }
}
