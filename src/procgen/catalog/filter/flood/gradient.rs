//! The `flood_gradient` op: a ramp across each island (Substance Designer's
//! *Flood Fill to Gradient*, #1741).

use serde::{Deserialize, Serialize};

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::flood::{self, Island};
use crate::procgen::recipe::{op, opt, Lit, OpInfo, ONE, SEED};

/// `flood_fill`'s islands → a 0→1 ramp across each island's bounding box, 0 off
/// them: tilted stones, slanted planks. The ramp runs along `angle` (turns; 0
/// runs left to right, 0.25 top to bottom), turned per island by up to
/// ±`angle_jitter` turns, drawn from the seed and the island. It spans the box
/// along that direction, so every island reaches 0 and 1 at its box's corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FloodGradient {
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub angle_jitter: f32,
    #[serde(default)]
    pub seed: u32,
}

impl Row for FloodGradient {
    const INFO: OpInfo = op(
        "flood_gradient",
        "filter",
        ONE,
        &[
            opt("angle", "number", Lit::Number(0.0)),
            opt("angle_jitter", "number", Lit::Number(0.0)),
            SEED,
        ],
    );
}

impl Op for FloodGradient {
    fn run(&self, cx: &Cx) -> Image {
        let islands = flood::of_label_image(cx.input(0));
        flood::paint(&islands, |i, u, v| {
            let turn = self.angle + self.angle_jitter * (2.0 * i.random(1, cx.seed) - 1.0);
            ramp(i, u, v, turn * std::f32::consts::TAU)
        })
    }

    fn seed(&self) -> u32 {
        self.seed
    }
}

/// The ramp at pixel centre `(u, v)` of `island`, along `theta` radians: the
/// pixel's offset from the box's centre (wrapped, so a box across an edge reads
/// one ramp) projected on the direction, over the box's reach along it.
fn ramp(island: &Island, u: f32, v: f32, theta: f32) -> f32 {
    let (dy, dx) = theta.sin_cos();
    let [cu, cv] = island.centre();
    let wrap = |d: f32| d - (d + 0.5).floor();
    let along = wrap(u - cu) * dx + wrap(v - cv) * dy;
    let reach = island.size[0] * dx.abs() + island.size[1] * dy.abs();
    if !(reach > 0.0) || !along.is_finite() {
        return 0.5;
    }
    (0.5 + along / reach).clamp(0.0, 1.0)
}
