//! src/procgen/recipe/tag.rs — an [`OpKind`]'s `op` tag, its catalog entry, its
//! node seed, the inputs it accepts and the params a link may drive.
//!
//! The tag is the string serde writes as `op`; `catalog_tests.rs` holds the two
//! equal for every op, so [`OpKind::info`] always finds its row in [`OPS`].

use super::arity::ONE;
use super::{Arity, Link, OpInfo, OpKind, Param, OPS};

impl OpKind {
    /// The op's recipe tag (`"noise"`, `"mix"`, …), as serde writes it.
    pub fn tag(&self) -> &'static str {
        match self {
            OpKind::Constant(_) => "constant",
            OpKind::Noise(_) => "noise",
            OpKind::Voronoi(_) => "voronoi",
            OpKind::Gradient(_) => "gradient",
            OpKind::Wave(_) => "wave",
            OpKind::Brick(_) => "brick",
            OpKind::Shape(_) => "shape",
            OpKind::Checker(_) => "checker",
            OpKind::WhiteNoise(_) => "white_noise",
            OpKind::Image(_) => "image",
            OpKind::ColorRamp(_) => "color_ramp",
            OpKind::Mix(_) => "mix",
            OpKind::Invert => "invert",
            OpKind::BrightContrast(_) => "bright_contrast",
            OpKind::HueSatValue(_) => "hue_sat_value",
            OpKind::Gamma(_) => "gamma",
            OpKind::AutoLevels => "auto_levels",
            OpKind::Mapping(_) => "mapping",
            OpKind::BumpToNormal(_) => "bump_to_normal",
            OpKind::CombineRgb => "combine_rgb",
            OpKind::SeparateRgb(_) => "separate_rgb",
            OpKind::Tile(_) => "tile",
            OpKind::Warp(_) => "warp",
            OpKind::NormalBlend(_) => "normal_blend",
            OpKind::Math(_) => "math",
            OpKind::MapRange(_) => "map_range",
            OpKind::Clamp(_) => "clamp",
            OpKind::RgbToBw => "rgb_to_bw",
            OpKind::Blur(_) => "blur",
            OpKind::Cavity(_) => "cavity",
            OpKind::Curvature(_) => "curvature",
            OpKind::Distance(_) => "distance",
        }
    }

    /// The node's own `seed` param (#1487), folded into the recipe seed by the
    /// runner; 0 for an op that draws no randomness.
    pub fn seed(&self) -> u32 {
        match self {
            OpKind::Noise(p) => p.seed,
            OpKind::Voronoi(p) => p.seed,
            OpKind::Brick(p) => p.seed,
            OpKind::WhiteNoise(p) => p.seed,
            OpKind::Tile(p) => p.seed,
            _ => 0,
        }
    }

    /// The node's socketable params with their names (#1494): the ones its
    /// catalog row marks `socket`, which a link may drive.
    pub fn sockets(&self) -> Vec<(&'static str, &Param)> {
        match self {
            OpKind::Noise(p) => vec![("gain", &p.gain)],
            OpKind::Voronoi(p) => vec![("randomness", &p.randomness)],
            OpKind::Shape(p) => vec![("softness", &p.softness)],
            OpKind::BrightContrast(p) => vec![("bright", &p.bright), ("contrast", &p.contrast)],
            OpKind::HueSatValue(p) => {
                vec![("hue", &p.hue), ("sat", &p.sat), ("value", &p.value)]
            }
            OpKind::Gamma(p) => vec![("gamma", &p.gamma)],
            OpKind::BumpToNormal(p) => vec![("strength", &p.strength)],
            OpKind::Tile(p) => vec![
                ("jitter", &p.jitter),
                ("rotation_jitter", &p.rotation_jitter),
                ("scale_jitter", &p.scale_jitter),
            ],
            OpKind::Warp(p) => vec![("strength", &p.strength)],
            OpKind::NormalBlend(p) => vec![("strength", &p.strength)],
            OpKind::Blur(p) => vec![("radius", &p.radius)],
            OpKind::Curvature(p) => vec![("strength", &p.strength)],
            OpKind::Distance(p) => vec![("radius", &p.radius)],
            _ => Vec::new(),
        }
    }

    /// The node's links, each with the param it drives, in [`Self::sockets`] order.
    pub fn links(&self) -> Vec<(&'static str, &Link)> {
        let sockets = self.sockets().into_iter();
        sockets
            .filter_map(|(name, p)| Some((name, p.link()?)))
            .collect()
    }

    /// The inputs this node accepts: its [`OPS`] row's, narrowed to one for a unary
    /// `math` func, which has no right-hand side to read a second input into.
    pub fn arity(&self) -> Arity {
        match self {
            OpKind::Math(p) if p.func.is_unary() => ONE,
            _ => self.info().inputs,
        }
    }

    /// The op's row in [`OPS`]: its category, input [`super::Arity`] and params.
    pub fn info(&self) -> &'static OpInfo {
        let tag = self.tag();
        OPS.iter()
            .find(|info| info.op == tag)
            .expect("every OpKind has an OPS row (catalog_tests.rs)")
    }
}
