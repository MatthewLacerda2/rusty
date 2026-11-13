//! The `gamma` op.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::color;
use crate::recipe::{op, req, socket, OpInfo, Param, ONE};

/// Per-channel gamma (`c^gamma`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gamma {
    pub gamma: Param,
}

impl Row for Gamma {
    const INFO: OpInfo = op("gamma", "color", ONE, &[socket(req("gamma", "number"))]);
}

impl Op for Gamma {
    fn run(&self, cx: &Cx) -> Image {
        color::gamma(cx.own(), cx.field(&self.gamma))
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        vec![("gamma", &self.gamma)]
    }
}
