//! src/procgen/recipe/catalog.rs — the self-describing op table (#411).
//!
//! serde can tell which params an [`OpKind`] takes but not their defaults, types or
//! input arity, so this static table carries that metadata for `Texture.Ops()` —
//! the agent asks the engine what it supports instead of trusting docs that can lag.
//! The arity is also the runner's: a node wired with fewer or more inputs than its
//! op's [`super::Arity`] is refused before anything evaluates (#1485).
//! `catalog_tests.rs` keeps it honest against the enum: every variant has an entry,
//! every entry's params are exactly the variant's serde fields, and every default
//! here is the one serde fills in. A param marked [`socket`] takes a link to another
//! node as well as a number (#1494); the tests hold that flag to what parses.

use super::arity::{arity, ONE, SOURCE};
use super::op_info::{choice, op, opt, req, socket, Lit, OpInfo, OpParam};

const BLEND: &[&str] = &[
    "mix",
    "add",
    "multiply",
    "screen",
    "overlay",
    "subtract",
    "difference",
    "lighten",
    "darken",
    "divide",
];
const MATH: &[&str] = &[
    "add", "subtract", "multiply", "divide", "power", "min", "max", "abs", "fract", "sqrt",
];

/// A stochastic op's own seed, folded into the recipe seed (#1487).
const SEED: OpParam = opt("seed", "integer", Lit::Integer(0));

/// Every op, in the order `docs/api/Texture.md` lists them.
pub const OPS: &[OpInfo] = &[
    op("constant", "generator", SOURCE, &[req("color", "color")]),
    op(
        "noise",
        "generator",
        SOURCE,
        &[
            choice("kind", &["perlin", "fbm", "ridged", "turbulence"]),
            req("scale", "number"),
            opt("octaves", "integer", Lit::Integer(1)),
            opt("lacunarity", "number", Lit::Number(2.0)),
            socket(opt("gain", "number", Lit::Number(0.5))),
            SEED,
        ],
    ),
    op(
        "voronoi",
        "generator",
        SOURCE,
        &[
            req("scale", "number"),
            OpParam {
                default: Some(Lit::Text("distance")),
                ..choice("output", &["distance", "cells", "f2", "edges"])
            },
            OpParam {
                default: Some(Lit::Text("euclidean")),
                ..choice("metric", &["euclidean", "manhattan", "chebyshev"])
            },
            socket(opt("randomness", "number", Lit::Number(1.0))),
            SEED,
        ],
    ),
    op(
        "gradient",
        "generator",
        SOURCE,
        &[choice("kind", &["linear", "linear_tiling", "radial"])],
    ),
    op(
        "wave",
        "generator",
        SOURCE,
        &[
            choice("kind", &["bands", "rings"]),
            req("frequency", "number"),
        ],
    ),
    op(
        "brick",
        "generator",
        SOURCE,
        &[
            req("rows", "number"),
            req("cols", "number"),
            opt("mortar", "number", Lit::Number(0.05)),
            OpParam {
                default: Some(Lit::Text("mask")),
                ..choice("output", &["mask", "random", "bevel"])
            },
            SEED,
        ],
    ),
    op(
        "shape",
        "generator",
        SOURCE,
        &[
            choice("kind", &["circle", "rect", "rounded_rect", "line"]),
            req("size", "vec2"),
            opt("roundness", "number", Lit::Number(0.25)),
            socket(opt("softness", "number", Lit::Number(0.0))),
        ],
    ),
    op(
        "checker",
        "generator",
        SOURCE,
        &[
            req("tiles", "integer"),
            req("color_a", "color"),
            req("color_b", "color"),
        ],
    ),
    op("white_noise", "generator", SOURCE, &[SEED]),
    op(
        "image",
        "generator",
        SOURCE,
        &[
            req("path", "path"),
            OpParam {
                default: Some(Lit::Text("srgb")),
                ..choice("encoding", &["srgb", "linear"])
            },
        ],
    ),
    op(
        "color_ramp",
        "color",
        ONE,
        &[
            req("stops", "stops"),
            OpParam {
                default: Some(Lit::Text("linear")),
                ..choice("interpolation", &["linear", "constant", "ease"])
            },
        ],
    ),
    op(
        "mix",
        "color",
        arity(2, 3),
        &[
            choice("mode", BLEND),
            opt("factor", "number", Lit::Number(1.0)),
        ],
    ),
    op("invert", "color", ONE, &[]),
    op(
        "bright_contrast",
        "color",
        ONE,
        &[
            socket(req("bright", "number")),
            socket(req("contrast", "number")),
        ],
    ),
    op(
        "hue_sat_value",
        "color",
        ONE,
        &[
            socket(req("hue", "number")),
            socket(req("sat", "number")),
            socket(req("value", "number")),
        ],
    ),
    op("gamma", "color", ONE, &[socket(req("gamma", "number"))]),
    op("auto_levels", "color", ONE, &[]),
    op(
        "mapping",
        "vector",
        ONE,
        &[
            req("scale", "vec2"),
            req("rotation", "number"),
            req("translation", "vec2"),
            opt("tiling", "bool", Lit::Bool(true)),
        ],
    ),
    op(
        "bump_to_normal",
        "vector",
        ONE,
        &[socket(req("strength", "number"))],
    ),
    op("combine_rgb", "vector", arity(1, 3), &[]),
    op("separate_rgb", "vector", ONE, &[req("channel", "integer")]),
    op(
        "tile",
        "vector",
        ONE,
        &[
            req("count", "number"),
            socket(opt("jitter", "number", Lit::Number(0.0))),
            socket(opt("rotation_jitter", "number", Lit::Number(0.0))),
            socket(opt("scale_jitter", "number", Lit::Number(0.0))),
            SEED,
        ],
    ),
    op(
        "warp",
        "vector",
        arity(2, 2),
        &[socket(req("strength", "number"))],
    ),
    op(
        "normal_blend",
        "vector",
        arity(2, 2),
        &[socket(opt("strength", "number", Lit::Number(1.0)))],
    ),
    op(
        "math",
        "math",
        arity(1, 2),
        &[
            choice("func", MATH),
            opt("value", "number", Lit::Number(0.5)),
        ],
    ),
    op(
        "map_range",
        "math",
        ONE,
        &[
            req("from_min", "number"),
            req("from_max", "number"),
            req("to_min", "number"),
            req("to_max", "number"),
        ],
    ),
    op(
        "clamp",
        "math",
        ONE,
        &[req("min", "number"), req("max", "number")],
    ),
    op("rgb_to_bw", "math", ONE, &[]),
    op(
        "blur",
        "filter",
        ONE,
        &[
            socket(req("radius", "number")),
            OpParam {
                default: Some(Lit::Text("gaussian")),
                ..choice("kind", &["box", "gaussian"])
            },
        ],
    ),
    op("cavity", "filter", ONE, &[req("radius", "number")]),
    op(
        "curvature",
        "filter",
        ONE,
        &[
            req("radius", "number"),
            socket(opt("strength", "number", Lit::Number(1.0))),
        ],
    ),
    op(
        "distance",
        "filter",
        ONE,
        &[socket(req("radius", "number"))],
    ),
];

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
