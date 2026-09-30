//! src/core/curve/gradient.rs — an RGBA gradient over `t ∈ [0, 1]`.
//!
//! Unity's shape: colour keys and alpha keys are separate lists, so fading alpha
//! never needs a colour key at the same `t`. Alpha reuses [`Curve`].

use serde::{Deserialize, Serialize};

use super::{lerp_keys, Curve};

/// One colour keyframe: RGB at normalised time `t`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorKey {
    pub t: f32,
    pub color: [f32; 3],
}

/// Colour + alpha over `t`. No colour keys is white; no alpha keys is opaque.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    #[serde(default)]
    pub color_keys: Vec<ColorKey>,
    #[serde(default)]
    pub alpha: Curve,
}

impl Gradient {
    /// White, alpha fading linearly from 1 to 0 — the pre-#439 particle look.
    pub fn fade_out() -> Self {
        Self {
            color_keys: Vec::new(),
            alpha: Curve::linear(1.0, 0.0),
        }
    }

    /// One constant RGBA everywhere.
    pub fn solid([r, g, b, a]: [f32; 4]) -> Self {
        Self::linear([r, g, b, a], [r, g, b, a])
    }

    /// A straight blend from `start` at `t = 0` to `end` at `t = 1`.
    pub fn linear(start: [f32; 4], end: [f32; 4]) -> Self {
        let key = |t, c: [f32; 4]| ColorKey {
            t,
            color: [c[0], c[1], c[2]],
        };
        Self {
            color_keys: vec![key(0.0, start), key(1.0, end)],
            alpha: Curve::linear(start[3], end[3]),
        }
    }

    /// Re-sort both key lists by `t`.
    pub fn sort(&mut self) {
        self.color_keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        self.alpha.sort();
    }

    /// RGBA at `t` (each key list clamped to its own span).
    pub fn evaluate(&self, t: f32) -> [f32; 4] {
        let [r, g, b] = self.rgb(t);
        [r, g, b, self.alpha.evaluate(t)]
    }

    fn rgb(&self, t: f32) -> [f32; 3] {
        let keys = &self.color_keys;
        let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
            return [1.0; 3];
        };
        if t <= first.t {
            return first.color;
        }
        if t >= last.t {
            return last.color;
        }
        let i = keys.partition_point(|k| k.t <= t);
        let (a, b) = (keys[i - 1], keys[i]);
        std::array::from_fn(|c| lerp_keys(t, (a.t, b.t), a.color[c], b.color[c]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_blends_colour_and_alpha_independently() {
        let g = Gradient {
            color_keys: vec![
                ColorKey {
                    t: 0.0,
                    color: [1.0, 0.0, 0.0],
                },
                ColorKey {
                    t: 1.0,
                    color: [0.0, 0.0, 1.0],
                },
            ],
            alpha: Curve::from_keys(&[(0.0, 1.0), (0.5, 1.0), (1.0, 0.0)]),
        };
        assert_eq!(g.evaluate(0.5), [0.5, 0.0, 0.5, 1.0]);
        assert_eq!(g.evaluate(0.75), [0.25, 0.0, 0.75, 0.5]);
        assert_eq!(Gradient::default().evaluate(0.4), [1.0; 4]);
        assert_eq!(Gradient::fade_out().evaluate(0.25), [1.0, 1.0, 1.0, 0.75]);
        let l = Gradient::linear([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(l.evaluate(0.5), [0.5, 0.0, 0.5, 0.5]);
        assert_eq!(Gradient::solid([0.2; 4]).evaluate(0.7), [0.2; 4]);
    }
}
