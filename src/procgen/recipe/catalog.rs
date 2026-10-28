//! src/procgen/recipe/catalog.rs — the self-describing op table (#411).
//!
//! serde can tell which params an [`OpKind`] takes but not their defaults, types or
//! input arity, so this static table carries that metadata for `Texture.Ops()` —
//! the agent asks the engine what it supports instead of trusting docs that can lag.
//! The arity is also the runner's: a node wired with fewer or more inputs than its
//! op's [`Arity`] is refused before anything evaluates (#1485).
//! `catalog_tests.rs` keeps it honest against the enum: every variant has an entry,
//! every entry's params are exactly the variant's serde fields, and every default
//! here is the one serde fills in.

use super::arity::{arity, Arity, ONE, SOURCE};

/// A param's literal default value, as the agent would write it in a recipe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lit {
    Number(f32),
    Integer(u32),
    Bool(bool),
    Text(&'static str),
}

/// One param of an op: its recipe key, value type, the allowed values when the type
/// is `"enum"`, and its default (`None` = required).
#[derive(Clone, Copy, Debug)]
pub struct OpParam {
    pub name: &'static str,
    /// `number` | `integer` | `bool` | `color` (RGBA array) | `vec2` | `enum` | `stops`.
    pub ty: &'static str,
    /// The accepted tags for an `enum` param; empty otherwise.
    pub values: &'static [&'static str],
    pub default: Option<Lit>,
}

/// One op: its `op` tag, menu category, the inputs it accepts, and its params.
#[derive(Clone, Copy, Debug)]
pub struct OpInfo {
    pub op: &'static str,
    /// `generator` | `color` | `vector` | `math` | `filter`.
    pub category: &'static str,
    pub inputs: Arity,
    pub params: &'static [OpParam],
}

const fn req(name: &'static str, ty: &'static str) -> OpParam {
    OpParam {
        name,
        ty,
        values: &[],
        default: None,
    }
}

const fn opt(name: &'static str, ty: &'static str, default: Lit) -> OpParam {
    OpParam {
        name,
        ty,
        values: &[],
        default: Some(default),
    }
}

const fn choice(name: &'static str, values: &'static [&'static str]) -> OpParam {
    OpParam {
        name,
        ty: "enum",
        values,
        default: None,
    }
}

const fn op(
    op: &'static str,
    category: &'static str,
    inputs: Arity,
    params: &'static [OpParam],
) -> OpInfo {
    OpInfo {
        op,
        category,
        inputs,
        params,
    }
}

const BLEND: &[&str] = &[
    "mix",
    "add",
    "multiply",
    "screen",
    "overlay",
    "subtract",
    "difference",
];
const MATH: &[&str] = &[
    "add", "subtract", "multiply", "divide", "power", "min", "max", "abs", "fract", "sqrt",
];

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
            opt("gain", "number", Lit::Number(0.5)),
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
            opt("randomness", "number", Lit::Number(1.0)),
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
            opt("softness", "number", Lit::Number(0.0)),
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
    op("white_noise", "generator", SOURCE, &[]),
    op("color_ramp", "color", ONE, &[req("stops", "stops")]),
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
        &[req("bright", "number"), req("contrast", "number")],
    ),
    op(
        "hue_sat_value",
        "color",
        ONE,
        &[
            req("hue", "number"),
            req("sat", "number"),
            req("value", "number"),
        ],
    ),
    op("gamma", "color", ONE, &[req("gamma", "number")]),
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
        &[req("strength", "number")],
    ),
    op("combine_rgb", "vector", arity(1, 3), &[]),
    op("separate_rgb", "vector", ONE, &[req("channel", "integer")]),
    op(
        "tile",
        "vector",
        ONE,
        &[
            req("count", "number"),
            opt("jitter", "number", Lit::Number(0.0)),
            opt("rotation_jitter", "number", Lit::Number(0.0)),
            opt("scale_jitter", "number", Lit::Number(0.0)),
        ],
    ),
    op("warp", "vector", arity(2, 2), &[req("strength", "number")]),
    op(
        "math",
        "math",
        ONE,
        &[choice("func", MATH), req("value", "number")],
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
    op("blur", "filter", ONE, &[req("radius", "number")]),
    op("cavity", "filter", ONE, &[req("radius", "number")]),
    op(
        "curvature",
        "filter",
        ONE,
        &[
            req("radius", "number"),
            opt("strength", "number", Lit::Number(1.0)),
        ],
    ),
];

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
