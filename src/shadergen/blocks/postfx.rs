//! src/shadergen/blocks/postfx.rs — the postfx building-block family (#272).
//!
//! Each block is a fullscreen effect over the tonemapped scene color. Its `helper` is a
//! pure function `fn pfx_<id>(c: vec3<f32>, uv: vec2<f32>, <params…>) -> vec3<f32>`
//! and its `call` chains the running color (`{prev}`), the fragment `uv` and the
//! instance's params (`{args}`). The assembler
//! emits the postfx scaffolding (the fullscreen-triangle `vs_fullscreen`, the
//! `t_color`/`s_color` bindings, an `fs_main` that samples the scene then folds
//! every block) so a baked postfx module has the self-contained fullscreen shape
//! the engine's postfx contract expects — it reads group(0) binding(0..2) just
//! like `postfx.wgsl`'s prefilter passes.
//!
//! Effects are deliberately bounded to per-pixel color grades that need only the
//! sampled color + uv, plus `game_time()` (#398) for animation (tint, vignette,
//! scanline, grayscale, posterize) — no new bindings, no neighbourhood taps, no new
//! render targets.
//!
//! The chain runs authored modules **after tonemapping** (#397), on display-referred
//! colour in `[0, 1]`. Exposure, saturation and contrast were blocks here too; they
//! were removed because the volume already owns those knobs (`Graphics.SetExposure`
//! and friends, applied in HDR where they belong), so this catalog holds only looks
//! the built-in grade can't make.

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
            runtime: false,
        }],
        helper: "fn pfx_tint(c: vec3<f32>, uv: vec2<f32>, color: vec3<f32>) -> vec3<f32> {\n    return c * color;\n}",
        call: "pfx_tint({prev}, uv{args})",
    },
    Block {
        id: "grayscale",
        desc: "Collapse to Rec.709 luminance.",
        params: &[],
        helper: "fn pfx_grayscale(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return vec3<f32>(l);\n}",
        call: "pfx_grayscale({prev}, uv{args})",
    },
    Block {
        id: "vignette",
        desc: "Darken toward the screen edges; strength and radius tune the falloff.",
        params: &[
            Param {
                name: "strength",
                default: 0.5,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "radius",
                default: 0.75,
                arity: 1,
                runtime: false,
            },
        ],
        helper: "fn pfx_vignette(c: vec3<f32>, uv: vec2<f32>, strength: f32, radius: f32) -> vec3<f32> {\n    let d = distance(uv, vec2<f32>(0.5));\n    let v = smoothstep(radius, radius * 0.5, d);\n    return c * mix(1.0, v, strength);\n}",
        call: "pfx_vignette({prev}, uv{args})",
    },
    Block {
        id: "scanline",
        desc: "CRT-style horizontal scanlines; count and strength tune the look.",
        params: &[
            Param {
                name: "count",
                default: 240.0,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "strength",
                default: 0.2,
                arity: 1,
                runtime: false,
            },
        ],
        helper: "fn pfx_scanline(c: vec3<f32>, uv: vec2<f32>, count: f32, strength: f32) -> vec3<f32> {\n    let s = sin(uv.y * count * 3.14159265);\n    let line = 1.0 - strength * (0.5 - 0.5 * s);\n    return c * line;\n}",
        call: "pfx_scanline({prev}, uv{args})",
    },
    Block {
        id: "posterize",
        desc: "Quantize the color into N bands per channel (retro / cel look).",
        params: &[Param {
            name: "levels",
            default: 6.0,
            arity: 1,
            runtime: false,
        }],
        helper: "fn pfx_posterize(c: vec3<f32>, uv: vec2<f32>, levels: f32) -> vec3<f32> {\n    let n = max(levels, 1.0);\n    return floor(c * n + 0.5) / n;\n}",
        call: "pfx_posterize({prev}, uv{args})",
    },
];
