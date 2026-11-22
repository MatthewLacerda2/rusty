//! One row per op, one file per op (`crates/procgen/src/CLAUDE.md`, *Adding an op*; the
//! test-only `dummy` is the worked example). `recipe`'s catalog and socket tests hold every
//! row to the parser.
// sorted: one module per family; an op's own enum is named through it (`catalog::<family>::<op>::<Enum>`)
pub(crate) mod color;
pub(crate) mod filter;
pub(crate) mod generators;
pub(crate) mod math;
mod op;
pub(crate) mod vector;

pub use op::{Cx, Op, Row};

use serde::{Deserialize, Serialize};

use super::graph::Instance;
use super::image_buf::Image;
use super::recipe::{Arity, Link, OpInfo, Param};

/// A row is `Variant(path::Params)`, or `Variant = path::Op` for a unit-struct op. `///` docs
/// then a `#[cfg(..)]` on a row carry to everything it lands on.
macro_rules! catalog {
    (@emit [$({$($pd:tt)*} {$($pc:tt)*} $pv:ident $pt:path;)*]
        [$({$($ud:tt)*} {$($uc:tt)*} $uv:ident $ut:path;)*]
        [$({$($ac:tt)*} $at:path;)*]) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "op", rename_all = "snake_case")]
        pub enum OpKind {
            $($($pd)* $($pc)* $pv($pt),)*
            $($($ud)* $($uc)* $uv,)*
            /// Not a row, so not in [`OPS`]: [`crate::graph::expand`] replaces it before anything runs.
            Graph(Instance),
        }

        $($($pc)* pub use $pt;)*

        pub const OPS: &[OpInfo] = &[$($($ac)* <$at as Row>::INFO,)*];

        impl OpKind {
            pub fn info(&self) -> &'static OpInfo {
                match self {
                    $($($pc)* OpKind::$pv(_) => &<$pt as Row>::INFO,)*
                    $($($uc)* OpKind::$uv => &<$ut as Row>::INFO,)*
                    OpKind::Graph(_) => &<Instance as Row>::INFO,
                }
            }

            fn op(&self) -> &dyn Op {
                match self {
                    $($($pc)* OpKind::$pv(p) => p,)*
                    $($($uc)* OpKind::$uv => &$ut,)*
                    OpKind::Graph(g) => g,
                }
            }
        }
    };
    (@emit [$($p:tt)*] [$($u:tt)*] [$($a:tt)*]
        $(#[doc = $d:literal])* $(#[cfg($c:meta)])? $v:ident($t:path), $($rest:tt)*) => {
        catalog!(@emit
            [$($p)* {$(#[doc = $d])*} {$(#[cfg($c)])?} $v $t;] [$($u)*]
            [$($a)* {$(#[cfg($c)])?} $t;] $($rest)*);
    };
    (@emit [$($p:tt)*] [$($u:tt)*] [$($a:tt)*]
        $(#[doc = $d:literal])* $(#[cfg($c:meta)])? $v:ident = $t:path, $($rest:tt)*) => {
        catalog!(@emit
            [$($p)*] [$($u)* {$(#[doc = $d])*} {$(#[cfg($c)])?} $v $t;]
            [$($a)* {$(#[cfg($c)])?} $t;] $($rest)*);
    };
    ($($rows:tt)*) => {
        catalog!(@emit [] [] [] $($rows)*);
    };
}

catalog! {
    // sorted: one row per op; registry order is only the order `Texture.Ops()` lists
    AmbientOcclusion(filter::relief::ambient_occlusion::AmbientOcclusion),
    AnisotropicNoise(generators::anisotropic_noise::AnisotropicNoise),
    AutoLevels = color::auto_levels::AutoLevels,
    Blur(filter::blurs::blur::Blur),
    Brick(generators::brick::Brick),
    BrightContrast(color::bright_contrast::BrightContrast),
    BumpToNormal(vector::bump_to_normal::BumpToNormal),
    Cavity(filter::relief::cavity::Cavity),
    Checker(generators::checker::Checker),
    Clamp(math::clamp::Clamp),
    ColorRamp(color::color_ramp::ColorRamp),
    /// A colour channel without an input reads 0, alpha reads 1.
    CombineRgb = vector::combine_rgb::CombineRgb,
    Constant(generators::constant::Constant),
    Coordinates = generators::coordinates::Coordinates,
    Curvature(filter::relief::curvature::Curvature),
    DirectionalBlur(filter::blurs::directional_blur::DirectionalBlur),
    DirectionalWarp(vector::directional_warp::DirectionalWarp),
    Distance(filter::mask::distance::Distance),
    #[cfg(test)]
    Dummy(color::dummy::Dummy),
    EdgeDetect(filter::mask::edge_detect::EdgeDetect),
    Expression(math::expression::Expression),
    FloodBbox(filter::flood::bbox::FloodBbox),
    FloodFill(filter::flood::fill::FloodFill),
    FloodGradient(filter::flood::gradient::FloodGradient),
    FloodRandom(filter::flood::random::FloodRandom),
    Gamma(color::gamma::Gamma),
    Gradient(generators::gradient::Gradient),
    Highpass(filter::blurs::highpass::Highpass),
    HistogramScan(color::histogram_scan::HistogramScan),
    HueSatValue(color::hue_sat_value::HueSatValue),
    Image(generators::image::ImageTexture),
    Invert = color::invert::Invert,
    Levels(color::levels::Levels),
    Mapping(vector::mapping::Mapping),
    MapRange(math::map_range::MapRange),
    Math(math::arithmetic::Math),
    Mix(color::mix::Mix),
    Noise(generators::noise::Noise),
    NormalBlend(vector::normal_blend::NormalBlend),
    Path(generators::outline::path::Path),
    RgbToBw = math::rgb_to_bw::RgbToBw,
    SampleAt = vector::sample_at::SampleAt,
    Scatter(vector::scatter::Scatter),
    SeparateRgb(vector::separate_rgb::SeparateRgb),
    Shape(generators::shape::Shape),
    Sharpen(filter::blurs::sharpen::Sharpen),
    SlopeBlur(filter::blurs::slope_blur::SlopeBlur),
    Text(generators::outline::text::Text),
    Tile(vector::tile::Tile),
    Voronoi(generators::voronoi::Voronoi),
    Warp(vector::warp::Warp),
    Wave(generators::wave::Wave),
    WhiteNoise(generators::white_noise::WhiteNoise),
}

impl OpKind {
    pub fn tag(&self) -> &'static str {
        self.info().op
    }

    pub fn run(&self, cx: &Cx) -> Image {
        self.op().run(cx)
    }

    pub fn seed(&self) -> u32 {
        self.op().seed()
    }

    pub fn sockets(&self) -> Vec<(&'static str, &Param)> {
        self.op().sockets()
    }

    pub fn links(&self) -> Vec<(&'static str, &Link)> {
        let sockets = self.sockets().into_iter();
        sockets
            .filter_map(|(name, p)| Some((name, p.link()?)))
            .collect()
    }

    pub fn arity(&self) -> Arity {
        self.op().arity().unwrap_or(self.info().inputs)
    }

    pub fn func(&self) -> Option<String> {
        self.op().func()
    }

    pub fn arity_fix(&self, given: usize) -> Option<String> {
        self.op().arity_fix(given)
    }
}
