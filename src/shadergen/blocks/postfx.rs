//! src/shadergen/blocks/postfx.rs — the postfx building-block family (#272).
//!
//! Each block is a fullscreen effect over the HDR scene color. Its `helper` is a
//! pure function `fn pfx_<id>(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32>` and its
//! `call` chains the running color (`{prev}`) and the fragment `uv`. The assembler
//! emits the postfx scaffolding (the fullscreen-triangle `vs_fullscreen`, the
//! `t_color`/`s_color` bindings, an `fs_main` that samples the scene then folds
//! every block) so a baked postfx module has the self-contained fullscreen shape
//! the engine's postfx contract expects — it reads group(0) binding(0..2) just
//! like `postfx.wgsl`'s prefilter passes.
//!
//! Effects are deliberately bounded to per-pixel color grades that need only the
//! sampled color + uv (tint, vignette, scanline, grayscale, posterize) — no new
//! bindings, no neighbourhood taps, no new render targets.

use super::{Block, Param};

/// The postfx block catalog.
pub const BLOCKS: &[Block] = &[
    Block {
        id: "tint",
        desc: "Multiply the color by an RGB tint.",
        params: &[Param {
            name: "color",
            default: 1.0,
            arity: 3,
        }],
        helper: "fn pfx_tint(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    return c * tint_color;\n}",
        call: "pfx_tint({prev}, uv)",
    },
    Block {
        id: "exposure",
        desc: "Scale brightness by 2^stops (a pre-tonemap exposure nudge).",
        params: &[Param {
            name: "stops",
            default: 0.0,
            arity: 1,
        }],
        helper: "fn pfx_exposure(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    return c * exp2(exposure_stops);\n}",
        call: "pfx_exposure({prev}, uv)",
    },
    Block {
        id: "saturation",
        desc: "Push or pull saturation around luminance (0 = grayscale, 1 = unchanged).",
        params: &[Param {
            name: "amount",
            default: 1.0,
            arity: 1,
        }],
        helper: "fn pfx_saturation(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return mix(vec3<f32>(l), c, saturation_amount);\n}",
        call: "pfx_saturation({prev}, uv)",
    },
    Block {
        id: "grayscale",
        desc: "Collapse to Rec.709 luminance.",
        params: &[],
        helper: "fn pfx_grayscale(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return vec3<f32>(l);\n}",
        call: "pfx_grayscale({prev}, uv)",
    },
    Block {
        id: "vignette",
        desc: "Darken toward the screen edges; strength and radius tune the falloff.",
        params: &[
            Param {
                name: "strength",
                default: 0.5,
                arity: 1,
            },
            Param {
                name: "radius",
                default: 0.75,
                arity: 1,
            },
        ],
        helper: "fn pfx_vignette(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let d = distance(uv, vec2<f32>(0.5));\n    let v = smoothstep(vignette_radius, vignette_radius * 0.5, d);\n    return c * mix(1.0, v, vignette_strength);\n}",
        call: "pfx_vignette({prev}, uv)",
    },
    Block {
        id: "scanline",
        desc: "CRT-style horizontal scanlines; count and strength tune the look.",
        params: &[
            Param {
                name: "count",
                default: 240.0,
                arity: 1,
            },
            Param {
                name: "strength",
                default: 0.2,
                arity: 1,
            },
        ],
        helper: "fn pfx_scanline(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let s = sin(uv.y * scanline_count * 3.14159265);\n    let line = 1.0 - scanline_strength * (0.5 - 0.5 * s);\n    return c * line;\n}",
        call: "pfx_scanline({prev}, uv)",
    },
    Block {
        id: "posterize",
        desc: "Quantize the color into N bands per channel (retro / cel look).",
        params: &[Param {
            name: "levels",
            default: 6.0,
            arity: 1,
        }],
        helper: "fn pfx_posterize(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let n = max(posterize_levels, 1.0);\n    return floor(c * n + 0.5) / n;\n}",
        call: "pfx_posterize({prev}, uv)",
    },
    Block {
        id: "contrast",
        desc: "Expand or compress contrast around mid-gray (0.5).",
        params: &[Param {
            name: "amount",
            default: 1.0,
            arity: 1,
        }],
        helper: "fn pfx_contrast(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    return (c - vec3<f32>(0.5)) * contrast_amount + vec3<f32>(0.5);\n}",
        call: "pfx_contrast({prev}, uv)",
    },
];
