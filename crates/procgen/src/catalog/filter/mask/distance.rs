use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::distance;
use crate::recipe::{op, req, socket, OpInfo, Param, ONE};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Distance {
    pub radius: Param,
}

impl Row for Distance {
    const INFO: OpInfo = op(
        "distance",
        "filter",
        ONE,
        &[socket(req("radius", "number"))],
    );
}

impl Op for Distance {
    fn run(&self, cx: &Cx) -> Image {
        distance::distance(cx.input(0), cx.field(&self.radius))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("radius", &self.radius)]
    }
}
