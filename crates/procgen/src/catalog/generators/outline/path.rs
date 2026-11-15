//! The `path` op: SVG path data filled (or stroked) into a mask.

use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators::clamp_or;
use crate::ops::outline::{self, fill, svg};
use crate::recipe::{choice, op, opt, req, Lit, OpInfo, OpParam, SOURCE};

/// Widest stroke, tiles.
const MAX_STROKE: f32 = 1.0;

/// How overlapping contours fill: SVG's `fill-rule`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    /// Inside wherever the outline winds around the point at all.
    #[default]
    Nonzero,
    /// Inside where it winds an odd number of times: a contour inside another
    /// cuts a hole whichever way it runs.
    Evenodd,
}

/// One outline from SVG path data, a mask: 1 inside, 0 outside, anti-aliased by
/// the exact share of each pixel it covers. `d` takes `M L H V C S Q T A Z`,
/// absolute and relative, in tile units with `v` down, as `coordinates` runs;
/// an open subpath fills as if closed. `stroke` above 0 draws the outline that
/// many tiles wide instead (round joins and caps), under `nonzero`. A shape that
/// crosses the tile's edge wraps. `d` is parsed with the recipe, so a malformed
/// one is refused, naming the byte, before anything bakes.
///
/// The fields are private: the parsed outline is built once from them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Written")]
pub struct Path {
    d: String,
    fill_rule: FillRule,
    stroke: f32,
    #[serde(skip)]
    subpaths: Vec<svg::Subpath>,
}

/// The params as the recipe writes them, before `d` parses.
#[derive(Deserialize)]
struct Written {
    d: String,
    #[serde(default)]
    fill_rule: FillRule,
    #[serde(default)]
    stroke: f32,
}

impl TryFrom<Written> for Path {
    type Error = String;

    fn try_from(w: Written) -> Result<Self, String> {
        let subpaths = svg::parse(&w.d).map_err(|e| format!("`d` {e}"))?;
        Ok(Self {
            d: w.d,
            fill_rule: w.fill_rule,
            stroke: w.stroke,
            subpaths,
        })
    }
}

impl Row for Path {
    const INFO: OpInfo = op(
        "path",
        "generator",
        SOURCE,
        &[
            req("d", "text"),
            OpParam {
                default: Some(Lit::Text("nonzero")),
                ..choice("fill_rule", &["nonzero", "evenodd"])
            },
            opt("stroke", "number", Lit::Number(0.0)),
        ],
    );
}

impl Op for Path {
    fn run(&self, cx: &Cx) -> Image {
        let width = clamp_or(self.stroke, 0.0, 0.0, MAX_STROKE);
        let (shapes, rule) = match width > 0.0 {
            true => (outline::stroke(&self.subpaths, width), FillRule::Nonzero),
            false => (outline::filled(&self.subpaths), self.fill_rule),
        };
        mask(&shapes, cx, rule)
    }
}

/// `shapes` filled under `rule` at the node's extent, as a gray image.
pub(crate) fn mask(shapes: &[fill::Shape], cx: &Cx, rule: FillRule) -> Image {
    let cover = fill::coverage(shapes, cx.size, rule);
    let mut img = Image::new(cx.size);
    for (p, c) in img.pixels_mut().iter_mut().zip(cover) {
        *p = [c, c, c, 1.0];
    }
    img
}
