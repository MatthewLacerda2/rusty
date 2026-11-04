//! The `flood_random` op: a random grey per island (Substance Designer's *Flood
//! Fill to Random Grayscale*, #1741).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::flood;
use crate::procgen::recipe::{op, OpInfo, ONE, SEED};

/// `flood_fill`'s islands → one flat grey per island, uniform in `[0, 1)`, 0 off
/// them: per-stone colour, height or roughness. An island's grey comes from the
/// seed and where its centroid sits, never from the order the scan found it.
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
        flood::paint(&islands, |i, _, _| i.random(0, cx.seed))
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}
