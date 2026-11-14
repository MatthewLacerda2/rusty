//! The easing curves a tween travels on (#424).
//!
//! Robert Penner's set — `quad`, `cubic`, `quart`, `expo`, `sine`, `back`,
//! `elastic`, `bounce`, each `_in` / `_out` / `_in_out` — plus `linear` and the
//! plain `ease_in` / `ease_out` / `ease_in_out` words. Names are snake_case to
//! match zimmer's keyframe easings (`back_out`, `ease_in_out`), so a sound fade
//! and a UI fade are written the same way.
//!
//! **The output is not clamped.** `back` and `elastic` pass their ends on the way
//! — that overshoot is the animation. A property that cannot leave its range
//! (an alpha, a colour channel) is clamped where it is written, by its authoring
//! op. Every curve starts at exactly 0 and lands on exactly 1.

use std::f64::consts::PI;

/// One of the three shapes a family is played in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    In,
    Out,
    InOut,
}

/// A curve family, written as its "ease in" shape; `Out` and `InOut` are derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Quad,
    Cubic,
    Quart,
    Expo,
    Sine,
    Back,
    Elastic,
    Bounce,
}

const FAMILIES: [(&str, Family); 8] = [
    ("quad", Family::Quad),
    ("cubic", Family::Cubic),
    ("quart", Family::Quart),
    ("expo", Family::Expo),
    ("sine", Family::Sine),
    ("back", Family::Back),
    ("elastic", Family::Elastic),
    ("bounce", Family::Bounce),
];

/// How far `back` pulls past its ends: Penner's constant, about a tenth of the move.
const BACK: f64 = 1.701_58;

/// A tween's easing curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Ease {
    #[default]
    Linear,
    Curve(Family, Mode),
}

impl Ease {
    /// Parse an easing name (`"linear"`, `"quad_out"`, `"back_in_out"`, …).
    pub(crate) fn parse(name: &str) -> Option<Self> {
        let plain = match name {
            "linear" => return Some(Self::Linear),
            "ease_in" => Some((Family::Quad, Mode::In)),
            "ease_out" => Some((Family::Quad, Mode::Out)),
            "ease_in_out" => Some((Family::Quad, Mode::InOut)),
            _ => None,
        };
        if let Some((family, mode)) = plain {
            return Some(Self::Curve(family, mode));
        }
        let (family, mode) = if let Some(f) = name.strip_suffix("_in_out") {
            (f, Mode::InOut)
        } else if let Some(f) = name.strip_suffix("_out") {
            (f, Mode::Out)
        } else {
            (name.strip_suffix("_in")?, Mode::In)
        };
        let family = FAMILIES.iter().find(|(n, _)| *n == family)?.1;
        Some(Self::Curve(family, mode))
    }

    /// Every accepted name, for the error a typo gets.
    pub(crate) fn names() -> String {
        let mut names = vec!["linear", "ease_in", "ease_out", "ease_in_out"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        for (family, _) in FAMILIES {
            names.push(format!("{family}_in|_out|_in_out"));
        }
        names.join(", ")
    }

    /// Reshape linear progress `p` (clamped to `0..=1`) into eased progress.
    pub(crate) fn apply(self, p: f64) -> f64 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Self::Linear => p,
            Self::Curve(family, Mode::In) => ease_in(family, p),
            Self::Curve(family, Mode::Out) => 1.0 - ease_in(family, 1.0 - p),
            Self::Curve(family, Mode::InOut) if p < 0.5 => ease_in(family, 2.0 * p) / 2.0,
            Self::Curve(family, Mode::InOut) => 1.0 - ease_in(family, 2.0 - 2.0 * p) / 2.0,
        }
    }
}

/// The family's "in" curve: 0 at `p = 0`, exactly 1 at `p = 1`.
fn ease_in(family: Family, p: f64) -> f64 {
    match family {
        Family::Quad => p * p,
        Family::Cubic => p * p * p,
        Family::Quart => p * p * p * p,
        Family::Expo if p <= 0.0 => 0.0,
        Family::Expo if p >= 1.0 => 1.0,
        Family::Expo => 2f64.powf(10.0 * p - 10.0),
        Family::Sine => 1.0 - (p * PI / 2.0).cos(),
        // `p² · ((c + 1)·p − c)`, ordered so `back(1)` is exactly 1.
        Family::Back => p * p * (BACK * (p - 1.0) + p),
        Family::Elastic if p <= 0.0 || p >= 1.0 => p,
        Family::Elastic => {
            -(2f64.powf(10.0 * p - 10.0)) * ((p * 10.0 - 10.75) * (2.0 * PI / 3.0)).sin()
        }
        Family::Bounce => 1.0 - bounce_out(1.0 - p),
    }
}

/// Penner's bounce, played "out": four ever-smaller hops landing on 1.
fn bounce_out(p: f64) -> f64 {
    const N: f64 = 7.5625;
    const D: f64 = 2.75;
    if p < 1.0 / D {
        N * p * p
    } else if p < 2.0 / D {
        let p = p - 1.5 / D;
        N * p * p + 0.75
    } else if p < 2.5 / D {
        let p = p - 2.25 / D;
        N * p * p + 0.9375
    } else {
        let p = p - 2.625 / D;
        N * p * p + 0.984_375
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Ease> {
        let mut eases = vec![Ease::Linear];
        for (family, _) in FAMILIES {
            for mode in ["in", "out", "in_out"] {
                eases.push(Ease::parse(&format!("{family}_{mode}")).unwrap());
            }
        }
        eases
    }

    #[test]
    fn every_curve_starts_at_zero_and_lands_on_one() {
        for ease in all() {
            assert!(ease.apply(0.0).abs() < 1e-9, "{ease:?} at 0");
            assert!((ease.apply(1.0) - 1.0).abs() < 1e-9, "{ease:?} at 1");
            let mid = ease.apply(0.5);
            assert!((-0.5..=1.5).contains(&mid), "{ease:?} mid {mid}");
        }
    }

    #[test]
    fn in_out_curves_are_symmetric_about_the_midpoint() {
        for ease in all() {
            if let Ease::Curve(_, Mode::InOut) = ease {
                let (a, b) = (ease.apply(0.3), ease.apply(0.7));
                assert!((a + b - 1.0).abs() < 1e-9, "{ease:?}");
            }
        }
    }

    #[test]
    fn back_out_overshoots_and_quad_does_not() {
        let peak = |e: Ease| {
            (1..100)
                .map(|i| e.apply(f64::from(i) / 100.0))
                .fold(0.0, f64::max)
        };
        assert!(peak(Ease::parse("back_out").unwrap()) > 1.05);
        assert!(peak(Ease::parse("elastic_out").unwrap()) > 1.0);
        assert!(peak(Ease::parse("quad_out").unwrap()) <= 1.0);
    }

    #[test]
    fn names_parse_and_typos_do_not() {
        assert_eq!(Ease::parse("ease_in_out"), Ease::parse("quad_in_out"));
        assert_eq!(Ease::parse("linear"), Some(Ease::Linear));
        for bad in ["", "quad", "Quad_in", "quad_sideways", "wobble_in", "_in"] {
            assert_eq!(Ease::parse(bad), None, "{bad}");
        }
        assert!(Ease::names().contains("bounce_in|_out|_in_out"));
    }
}
