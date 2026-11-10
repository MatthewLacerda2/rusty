//! The `color_ramp` op: Blender's ColorRamp.

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::ramp;
use crate::procgen::recipe::{
    choice, op, req, Lit, OpInfo, OpParam, RampInterpolation, RampStop, ONE,
};

/// Map the input's red channel through a gradient of stops, filled between them by
/// `interpolation` (Blender's ColorRamp).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorRamp {
    pub stops: Vec<RampStop>,
    #[serde(default)]
    pub interpolation: RampInterpolation,
}

impl Row for ColorRamp {
    const INFO: OpInfo = op(
        "color_ramp",
        "color",
        ONE,
        &[
            req("stops", "stops"),
            OpParam {
                default: Some(Lit::Text("linear")),
                ..choice("interpolation", &["linear", "constant", "ease"])
            },
        ],
    );
}

impl Op for ColorRamp {
    fn run(&self, cx: &Cx) -> Image {
        ramp::color_ramp(cx.own(), &self.stops, self.interpolation)
    }
}
