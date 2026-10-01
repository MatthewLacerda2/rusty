//! src/procgen/recipe/catalog.rs — the self-describing op table (#411).
//!
//! serde can tell which params an [`OpKind`] takes but not their defaults, types or
//! input arity, so this static table carries that metadata for `Texture.Ops()` —
//! the agent asks the engine what it supports instead of trusting docs that can lag.
//! `catalog_tests.rs` keeps it honest against the enum: every variant has an entry,
//! every entry's params are exactly the variant's serde fields, and every default
//! here is the one serde fills in.

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

/// One op: its `op` tag, menu category, how many inputs it consumes, and its params.
#[derive(Clone, Copy, Debug)]
pub struct OpInfo {
    pub op: &'static str,
    /// `generator` | `color` | `vector` | `math` | `filter`.
    pub category: &'static str,
    pub inputs: u8,
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
    inputs: u8,
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
    op("constant", "generator", 0, &[req("color", "color")]),
    op(
        "noise",
        "generator",
        0,
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
        0,
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
        0,
        &[choice("kind", &["linear", "linear_tiling", "radial"])],
    ),
    op(
        "wave",
        "generator",
        0,
        &[
            choice("kind", &["bands", "rings"]),
            req("frequency", "number"),
        ],
    ),
    op(
        "brick",
        "generator",
        0,
        &[
            req("rows", "number"),
            req("cols", "number"),
            opt("mortar", "number", Lit::Number(0.05)),
        ],
    ),
    op(
        "checker",
        "generator",
        0,
        &[
            req("tiles", "integer"),
            req("color_a", "color"),
            req("color_b", "color"),
        ],
    ),
    op("white_noise", "generator", 0, &[]),
    op("color_ramp", "color", 1, &[req("stops", "stops")]),
    op(
        "mix",
        "color",
        2,
        &[choice("mode", BLEND), req("factor", "number")],
    ),
    op("invert", "color", 1, &[]),
    op(
        "bright_contrast",
        "color",
        1,
        &[req("bright", "number"), req("contrast", "number")],
    ),
    op(
        "hue_sat_value",
        "color",
        1,
        &[
            req("hue", "number"),
            req("sat", "number"),
            req("value", "number"),
        ],
    ),
    op("gamma", "color", 1, &[req("gamma", "number")]),
    op(
        "mapping",
        "vector",
        1,
        &[
            req("scale", "vec2"),
            req("rotation", "number"),
            req("translation", "vec2"),
            opt("tiling", "bool", Lit::Bool(true)),
        ],
    ),
    op("bump_to_normal", "vector", 1, &[req("strength", "number")]),
    op("combine_rgb", "vector", 3, &[]),
    op("separate_rgb", "vector", 1, &[req("channel", "integer")]),
    op("warp", "vector", 2, &[req("strength", "number")]),
    op(
        "math",
        "math",
        1,
        &[choice("func", MATH), req("value", "number")],
    ),
    op(
        "map_range",
        "math",
        1,
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
        1,
        &[req("min", "number"), req("max", "number")],
    ),
    op("rgb_to_bw", "math", 1, &[]),
    op("blur", "filter", 1, &[req("radius", "number")]),
];

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
