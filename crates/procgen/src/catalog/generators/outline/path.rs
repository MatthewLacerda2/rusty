use serde::{Deserialize, Serialize};

use crate::catalog::{Cx, Op, Row};
use crate::image_buf::Image;
use crate::ops::generators::clamp_or;
use crate::ops::outline::{self, fill, stroke, svg};
use crate::recipe::{choice, op, opt, req_sampled, Lit, OpInfo, OpParam, SOURCE};

/// The widest stroke, in tiles.
const MAX_STROKE: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    #[default]
    Nonzero,
    Evenodd,
}

/// The fields are private: the parsed `subpaths` are built once from them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Written")]
pub struct Path {
    d: String,
    fill_rule: FillRule,
    stroke: f32,
    #[serde(skip)]
    subpaths: Vec<svg::Subpath>,
}

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
            req_sampled("d", "M0.25 0.25 L0.75 0.25 L0.5 0.75 Z"),
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
        let cover = match width > 0.0 {
            true => stroke::coverage(&self.subpaths, width, cx.size),
            false => fill::coverage(&outline::filled(&self.subpaths), cx.size, self.fill_rule),
        };
        gray(cover, cx)
    }
}

pub(crate) fn mask(shapes: &[fill::Shape], cx: &Cx, rule: FillRule) -> Image {
    gray(fill::coverage(shapes, cx.size, rule), cx)
}

fn gray(cover: Vec<f32>, cx: &Cx) -> Image {
    let mut img = Image::new(cx.size);
    for (p, c) in img.pixels_mut().iter_mut().zip(cover) {
        *p = [c, c, c, 1.0];
    }
    img
}
