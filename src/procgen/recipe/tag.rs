//! src/procgen/recipe/tag.rs — an [`OpKind`]'s `op` tag and its catalog entry.
//!
//! The tag is the string serde writes as `op`; `catalog_tests.rs` holds the two
//! equal for every op, so [`OpKind::info`] always finds its row in [`OPS`].

use super::{OpInfo, OpKind, OPS};

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
            OpKind::WhiteNoise => "white_noise",
            OpKind::ColorRamp(_) => "color_ramp",
            OpKind::Mix(_) => "mix",
            OpKind::Invert => "invert",
            OpKind::BrightContrast(_) => "bright_contrast",
            OpKind::HueSatValue(_) => "hue_sat_value",
            OpKind::Gamma(_) => "gamma",
            OpKind::Mapping(_) => "mapping",
            OpKind::BumpToNormal(_) => "bump_to_normal",
            OpKind::CombineRgb => "combine_rgb",
            OpKind::SeparateRgb(_) => "separate_rgb",
            OpKind::Tile(_) => "tile",
            OpKind::Warp(_) => "warp",
            OpKind::Math(_) => "math",
            OpKind::MapRange(_) => "map_range",
            OpKind::Clamp(_) => "clamp",
            OpKind::RgbToBw => "rgb_to_bw",
            OpKind::Blur(_) => "blur",
            OpKind::Cavity(_) => "cavity",
            OpKind::Curvature(_) => "curvature",
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
