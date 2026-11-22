use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::filter;
use crate::recipe::{choice, op, req, socket, BlurKind, Lit, OpInfo, OpParam, Param, ONE};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blur {
    pub radius: Param,
    #[serde(default)]
    pub kind: BlurKind,
}

impl Row for Blur {
    const INFO: OpInfo = op(
        "blur",
        "filter",
        ONE,
        &[
            socket(req("radius", "number")),
            OpParam {
                default: Some(Lit::Text("gaussian")),
                ..choice("kind", &["box", "gaussian"])
            },
        ],
    );
}

impl Op for Blur {
    fn run(&self, cx: &Cx) -> Image {
        filter::blur_varying(cx.input(0), cx.field(&self.radius), self.kind)
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("radius", &self.radius)]
    }
}
