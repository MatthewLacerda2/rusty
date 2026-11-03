//! src/procgen/catalog/mod.rs — the op registry (#1751): every op Snyder runs, one
//! row each.
//!
//! An op is **one file** under its family's folder (`generators/`, `color/`,
//! `vector/`, `math/`, `filter/`): its params struct (serde, the recipe's fields),
//! its [`Row`] (the catalog entry `Texture.Ops()` lists: tag, category, arity,
//! params with their defaults) and its [`Op`] (how it runs, its seed, its sockets).
//! The row below names its [`OpKind`] variant and that struct, and the macro derives
//! everything a shared list used to repeat: the enum serde parses, [`OPS`], and the
//! dispatch. Adding an op is its file, its `mod` line in the family, and its row
//! here (`src/procgen/CLAUDE.md`, *Adding an op*; the test-only `dummy` is the
//! worked example).
//!
//! The math an op runs may sit in its own file, or in [`super::ops`] when siblings
//! share it (the blur kernels under `cavity` and `blur`).
//!
//! serde can tell which params an op takes but not their defaults, types or input
//! arity, so each [`Row`] carries that for `Texture.Ops()` (#411); the arity is also
//! the runner's (#1485). `recipe`'s `catalog_tests.rs` and `socket_tests.rs` hold
//! every row to the parser: its tag, its params, each default, each enum's values
//! and which params take a link.

mod color;
mod filter;
mod generators;
mod math;
mod op;
mod vector;

pub use op::{Cx, Op, Row};

use serde::{Deserialize, Serialize};

use super::image_buf::Image;
use super::recipe::{Arity, Link, OpInfo, Param};

/// Builds [`OpKind`], its re-exported params structs and [`OPS`] from the rows,
/// in row order. A row is `Variant(path::Params)` for an op with params, or
/// `Variant = path::Op` for one without (a unit struct; the variant carries
/// nothing, so the recipe writes only its `op`). Attributes (`#[cfg(test)]`)
/// carry through to every place the row lands.
macro_rules! catalog {
    (@emit [$($(#[$pm:meta])* $pv:ident $pt:path;)*]
        [$($(#[$um:meta])* $uv:ident $ut:path;)*]
        [$($(#[$am:meta])* $at:path;)*]) => {
        /// The curated op-set ("Blender-lite"), one variant per [`catalog!`] row.
        /// serde writes the variant's snake_case name as the node's `op` tag, and its
        /// params struct's fields beside it.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "op", rename_all = "snake_case")]
        pub enum OpKind {
            $($(#[$pm])* $pv($pt),)*
            $($(#[$um])* $uv,)*
        }

        $($(#[$pm])* pub use $pt;)*

        /// Every op's catalog row, in registry order: what `Texture.Ops()` lists.
        pub const OPS: &[OpInfo] = &[$($(#[$am])* <$at as Row>::INFO,)*];

        impl OpKind {
            /// The op's row in [`OPS`]: its tag, category, input [`Arity`] and params.
            pub fn info(&self) -> &'static OpInfo {
                match self {
                    $($(#[$pm])* OpKind::$pv(_) => &<$pt as Row>::INFO,)*
                    $($(#[$um])* OpKind::$uv => &<$ut as Row>::INFO,)*
                }
            }

            /// The op's behaviour.
            fn op(&self) -> &dyn Op {
                match self {
                    $($(#[$pm])* OpKind::$pv(p) => p,)*
                    $($(#[$um])* OpKind::$uv => &$ut,)*
                }
            }
        }
    };
    (@emit [$($p:tt)*] [$($u:tt)*] [$($a:tt)*]
        $(#[$m:meta])* $v:ident($t:path), $($rest:tt)*) => {
        catalog!(@emit [$($p)* $(#[$m])* $v $t;] [$($u)*] [$($a)* $(#[$m])* $t;] $($rest)*);
    };
    (@emit [$($p:tt)*] [$($u:tt)*] [$($a:tt)*]
        $(#[$m:meta])* $v:ident = $t:path, $($rest:tt)*) => {
        catalog!(@emit [$($p)*] [$($u)* $(#[$m])* $v $t;] [$($a)* $(#[$m])* $t;] $($rest)*);
    };
    ($($rows:tt)*) => {
        catalog!(@emit [] [] [] $($rows)*);
    };
}

catalog! {
    // sorted: one row per op; registry order is only the order `Texture.Ops()` lists
    AutoLevels = color::auto_levels::AutoLevels,
    Blur(filter::blur::Blur),
    Brick(generators::brick::Brick),
    BrightContrast(color::bright_contrast::BrightContrast),
    BumpToNormal(vector::bump_to_normal::BumpToNormal),
    Cavity(filter::cavity::Cavity),
    Checker(generators::checker::Checker),
    Clamp(math::clamp::Clamp),
    ColorRamp(color::color_ramp::ColorRamp),
    CombineRgb = vector::combine_rgb::CombineRgb,
    Constant(generators::constant::Constant),
    Curvature(filter::curvature::Curvature),
    Distance(filter::distance::Distance),
    #[cfg(test)]
    Dummy(color::dummy::Dummy),
    Gamma(color::gamma::Gamma),
    Gradient(generators::gradient::Gradient),
    HueSatValue(color::hue_sat_value::HueSatValue),
    Image(generators::image::ImageTexture),
    Invert = color::invert::Invert,
    MapRange(math::map_range::MapRange),
    Mapping(vector::mapping::Mapping),
    Math(math::math::Math),
    Mix(color::mix::Mix),
    Noise(generators::noise::Noise),
    NormalBlend(vector::normal_blend::NormalBlend),
    RgbToBw = math::rgb_to_bw::RgbToBw,
    SeparateRgb(vector::separate_rgb::SeparateRgb),
    Shape(generators::shape::Shape),
    Tile(vector::tile::Tile),
    Voronoi(generators::voronoi::Voronoi),
    Warp(vector::warp::Warp),
    Wave(generators::wave::Wave),
    WhiteNoise(generators::white_noise::WhiteNoise),
}

impl OpKind {
    /// The op's recipe tag (`"noise"`, `"mix"`, …), as serde writes it.
    pub fn tag(&self) -> &'static str {
        self.info().op
    }

    /// Run the node: its output image.
    pub fn run(&self, cx: &Cx) -> Image {
        self.op().run(cx)
    }

    /// The node's own `seed` param (#1487), folded into the recipe seed by the
    /// runner; 0 for an op that draws no randomness.
    pub fn seed(&self) -> u32 {
        self.op().seed()
    }

    /// The node's socketable params with their names (#1494): the ones its
    /// catalog row marks `socket`, which a link may drive.
    pub fn sockets(&self) -> Vec<(&'static str, &Param)> {
        self.op().sockets()
    }

    /// The node's links, each with the param it drives, in [`Self::sockets`] order.
    pub fn links(&self) -> Vec<(&'static str, &Link)> {
        let sockets = self.sockets().into_iter();
        sockets
            .filter_map(|(name, p)| Some((name, p.link()?)))
            .collect()
    }

    /// The inputs this node accepts: its [`OPS`] row's, unless a param narrows it
    /// (a unary `math` func has no right-hand side to read a second input into).
    pub fn arity(&self) -> Arity {
        self.op().arity().unwrap_or(self.info().inputs)
    }

    /// The sub-function an arity error about this node names (`math`'s `func`).
    pub fn func(&self) -> Option<String> {
        self.op().func()
    }
}
