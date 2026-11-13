//! The `hue_sat_value` op.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::color;
use crate::recipe::{op, req, socket, OpInfo, Param, ONE};

/// Hue rotate (turns), saturation and value scale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HueSatValue {
    pub hue: Param,
    pub sat: Param,
    pub value: Param,
}

impl Row for HueSatValue {
    const INFO: OpInfo = op(
        "hue_sat_value",
        "color",
        ONE,
        &[
            socket(req("hue", "number")),
            socket(req("sat", "number")),
            socket(req("value", "number")),
        ],
    );
}

impl Op for HueSatValue {
    fn run(&self, cx: &Cx) -> Image {
        let (hue, sat, value) = (
            cx.field(&self.hue),
            cx.field(&self.sat),
            cx.field(&self.value),
        );
        color::hue_sat_value(cx.own(), hue, sat, value)
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![
            ("hue", &self.hue),
            ("sat", &self.sat),
            ("value", &self.value),
        ]
    }
}
