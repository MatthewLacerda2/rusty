use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::filter;
use crate::recipe::{op, req, OpInfo, ONE};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cavity {
    pub radius: f32,
}

impl Row for Cavity {
    const INFO: OpInfo = op("cavity", "filter", ONE, &[req("radius", "number")]);
}

impl Op for Cavity {
    fn run(&self, cx: &Cx) -> Image {
        filter::cavity(cx.input(0), cx.px(self.radius))
    }
}
