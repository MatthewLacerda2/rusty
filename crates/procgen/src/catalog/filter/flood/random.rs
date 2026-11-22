use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::flood;
use crate::recipe::{op, OpInfo, ONE, SEED};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloodRandom {
    #[serde(default)]
    pub seed: u32,
}

impl Row for FloodRandom {
    const INFO: OpInfo = op("flood_random", "filter", ONE, &[SEED]);
}

impl Op for FloodRandom {
    fn run(&self, cx: &Cx) -> Image {
        let islands = flood::of_label_image(cx.input(0));
        flood::paint(&islands, |i| i.random(0, cx.seed), |g, _, _| *g)
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}
