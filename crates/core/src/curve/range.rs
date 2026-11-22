//! Sampled from the caller's unit draw `u` in `[0, 1)`, so a range owns no randomness:
//! the caller's seeded stream does.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "RangeRepr", into = "RangeRepr")]
pub struct Range {
    pub min: f32,
    pub max: f32,
}

impl Range {
    pub const fn constant(v: f32) -> Self {
        Self { min: v, max: v }
    }

    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn sample(&self, u: f32) -> f32 {
        self.min + (self.max - self.min) * u
    }

    pub fn is_constant(&self) -> bool {
        self.min == self.max
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum RangeRepr {
    Constant(f32),
    Span { min: f32, max: f32 },
}

impl From<RangeRepr> for Range {
    fn from(r: RangeRepr) -> Self {
        match r {
            RangeRepr::Constant(v) => Self::constant(v),
            RangeRepr::Span { min, max } => Self { min, max },
        }
    }
}

impl From<Range> for RangeRepr {
    fn from(r: Range) -> Self {
        match r.is_constant() {
            true => Self::Constant(r.min),
            false => Self::Span {
                min: r.min,
                max: r.max,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "ColorRangeRepr", into = "ColorRangeRepr")]
pub struct ColorRange {
    pub min: [f32; 4],
    pub max: [f32; 4],
}

impl ColorRange {
    pub const fn constant(c: [f32; 4]) -> Self {
        Self { min: c, max: c }
    }

    pub fn sample(&self, u: f32) -> [f32; 4] {
        std::array::from_fn(|i| self.min[i] + (self.max[i] - self.min[i]) * u)
    }

    pub fn is_constant(&self) -> bool {
        self.min == self.max
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ColorRangeRepr {
    Constant([f32; 4]),
    Span { min: [f32; 4], max: [f32; 4] },
}

impl From<ColorRangeRepr> for ColorRange {
    fn from(r: ColorRangeRepr) -> Self {
        match r {
            ColorRangeRepr::Constant(c) => Self::constant(c),
            ColorRangeRepr::Span { min, max } => Self { min, max },
        }
    }
}

impl From<ColorRange> for ColorRangeRepr {
    fn from(r: ColorRange) -> Self {
        match r.is_constant() {
            true => Self::Constant(r.min),
            false => Self::Span {
                min: r.min,
                max: r.max,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_sample_and_constants_are_bare_scalars() {
        let r = Range::new(2.0, 4.0);
        assert_eq!(r.sample(0.5), 3.0);
        let constant: Range = serde_json::from_str("1.5").unwrap();
        assert_eq!(constant, Range::constant(1.5));
        assert_eq!(serde_json::to_string(&constant).unwrap(), "1.5");
        let span: Range = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(span, r);
        let c: ColorRange = serde_json::from_str("[1.0,0.5,0.0,1.0]").unwrap();
        assert_eq!(c, ColorRange::constant([1.0, 0.5, 0.0, 1.0]));
        let c2 = ColorRange {
            min: [0.0; 4],
            max: [1.0; 4],
        };
        assert_eq!(c2.sample(0.25), [0.25; 4]);
    }
}
