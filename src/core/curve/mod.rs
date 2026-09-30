//! src/core/curve/ — small keyframe data shared by any system that varies a value
//! over a normalised `t ∈ [0, 1]` (#439): [`Curve`] (a scalar), [`Gradient`] (an
//! RGBA colour) and the random start ranges ([`Range`], [`ColorRange`]).
//!
//! Particles are the first user (size / colour / drag / rotation speed over a
//! particle's life); trail width or audio fades can reuse the same types. They are
//! plain serde data — keys, not closures — so a scene saves them and the inspector
//! and the API edit them directly. Evaluation is pure and deterministic.

mod gradient;
mod range;

pub use gradient::{ColorKey, Gradient};
pub use range::{ColorRange, Range};

use serde::{Deserialize, Serialize};

/// One keyframe: `value` at normalised time `t`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub t: f32,
    pub value: f32,
}

/// A piecewise-linear scalar curve over `t ∈ [0, 1]`. Keys are kept sorted by `t`;
/// before the first key it holds the first value, after the last the last value.
/// An empty curve is the identity multiplier `1`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub keys: Vec<Key>,
}

impl Curve {
    /// A flat curve at `value`.
    pub fn constant(value: f32) -> Self {
        Self {
            keys: vec![Key { t: 0.0, value }],
        }
    }

    /// A straight line from `start` at `t = 0` to `end` at `t = 1`.
    pub fn linear(start: f32, end: f32) -> Self {
        Self::from_keys(&[(0.0, start), (1.0, end)])
    }

    /// A curve through `(t, value)` pairs, in any order.
    pub fn from_keys(pairs: &[(f32, f32)]) -> Self {
        let mut curve = Self {
            keys: pairs.iter().map(|&(t, value)| Key { t, value }).collect(),
        };
        curve.sort();
        curve
    }

    /// Re-sort the keys by `t` (after an edit moved one past a neighbour).
    pub fn sort(&mut self) {
        self.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    }

    /// The value at `t` (clamped to the key span).
    pub fn evaluate(&self, t: f32) -> f32 {
        let keys = &self.keys;
        let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
            return 1.0;
        };
        if t <= first.t {
            return first.value;
        }
        if t >= last.t {
            return last.value;
        }
        let i = keys.partition_point(|k| k.t <= t);
        lerp_keys(
            t,
            (keys[i - 1].t, keys[i].t),
            keys[i - 1].value,
            keys[i].value,
        )
    }
}

/// Linear blend at `t` between `a` (at `span.0`) and `b` (at `span.1`).
pub(crate) fn lerp_keys(t: f32, span: (f32, f32), a: f32, b: f32) -> f32 {
    let width = span.1 - span.0;
    let s = if width > 0.0 {
        (t - span.0) / width
    } else {
        1.0
    };
    a + (b - a) * s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_interpolates_and_clamps() {
        let c = Curve::from_keys(&[(1.0, 0.0), (0.0, 2.0), (0.5, 4.0)]);
        assert_eq!(c.evaluate(-1.0), 2.0);
        assert_eq!(c.evaluate(0.25), 3.0);
        assert_eq!(c.evaluate(0.5), 4.0);
        assert_eq!(c.evaluate(0.75), 2.0);
        assert_eq!(c.evaluate(2.0), 0.0);
        assert_eq!(Curve::default().evaluate(0.3), 1.0, "empty is identity");
        assert_eq!(Curve::constant(0.5).evaluate(0.9), 0.5);
    }

    #[test]
    fn curve_round_trips_through_json() {
        let c = Curve::linear(1.0, 3.0);
        let back: Curve = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }
}
