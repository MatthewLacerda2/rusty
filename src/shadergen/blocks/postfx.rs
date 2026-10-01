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
//! Most effects are per-pixel color grades that need only the sampled color + uv,
//! plus `game_time()` (#398) for animation (tint, vignette, scanline, grayscale,
//! posterize, film grain, damage vignette). **Sampling blocks** (#402: chromatic
//! aberration, sharpen, radial blur) also tap the module's input at offsets through
//! the scaffold's `source_tap(uv)` / `source_texel()`. They read the module's
//! *input* — the colour the chain handed this module — not the running colour of
//! the blocks before them, and add the offset taps' **difference** from the
//! centre tap to the running colour. As the first block that is exactly the
//! re-sampled effect; later in the chain it layers the same fringe or detail over
//! whatever the earlier blocks graded. No new bindings, no new render targets.
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
        textures: &[],
    },
    Block {
        id: "grayscale",
        desc: "Collapse to Rec.709 luminance.",
        params: &[],
        helper: "fn pfx_grayscale(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return vec3<f32>(l);\n}",
        call: "pfx_grayscale({prev}, uv{args})",
        textures: &[],
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
        textures: &[],
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
        textures: &[],
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
        textures: &[],
    },
    Block {
        id: "film_grain",
        desc: "Per-pixel noise that changes every frame (hashed from pixel and game time).",
        params: &[Param {
            name: "strength",
            default: 0.06,
            arity: 1,
            runtime: false,
        }],
        helper: "fn pfx_film_grain(c: vec3<f32>, uv: vec2<f32>, strength: f32) -> vec3<f32> {\n    let px = vec2<u32>(uv / source_texel());\n    var h = (px.x * 73856093u) ^ (px.y * 19349663u) ^ (bitcast<u32>(game_time()) * 83492791u);\n    h = (h ^ (h >> 16u)) * 2246822519u;\n    h = (h ^ (h >> 13u)) * 3266489917u;\n    h = h ^ (h >> 16u);\n    let n = f32(h) / 4294967295.0 - 0.5;\n    return max(c + vec3<f32>(n * strength), vec3<f32>(0.0));\n}",
        call: "pfx_film_grain({prev}, uv{args})",
        textures: &[],
    },
    Block {
        id: "damage_vignette",
        desc: "Edge vignette blended toward a color (set it, e.g. red); pulses at pulse_speed.",
        params: &[
            Param {
                name: "color",
                default: 1.0,
                arity: 3,
            runtime: false,
            },
            Param {
                name: "intensity",
                default: 0.5,
                arity: 1,
            runtime: false,
            },
            Param {
                name: "pulse_speed",
                default: 0.0,
                arity: 1,
            runtime: false,
            },
        ],
        helper: "fn pfx_damage_vignette(c: vec3<f32>, uv: vec2<f32>, color: vec3<f32>, intensity: f32, pulse_speed: f32) -> vec3<f32> {\n    let edge = smoothstep(0.25, 0.75, distance(uv, vec2<f32>(0.5)));\n    let pulse = 0.75 + 0.25 * cos(game_time() * pulse_speed);\n    return mix(c, color, clamp(edge * intensity * pulse, 0.0, 1.0));\n}",
        call: "pfx_damage_vignette({prev}, uv{args})",
        textures: &[],
    },
    Block {
        id: "chromatic_aberration",
        desc: "Split R and B radially from the screen centre (samples the input).",
        params: &[Param {
            name: "strength",
            default: 0.01,
            arity: 1,
            runtime: false,
        }],
        helper: "fn pfx_chromatic_aberration(c: vec3<f32>, uv: vec2<f32>, strength: f32) -> vec3<f32> {\n    let off = (uv - vec2<f32>(0.5)) * strength;\n    let base = source_tap(uv);\n    let r = source_tap(uv + off).r - base.r;\n    let b = source_tap(uv - off).b - base.b;\n    return max(c + vec3<f32>(r, 0.0, b), vec3<f32>(0.0));\n}",
        call: "pfx_chromatic_aberration({prev}, uv{args})",
        textures: &[],
    },
    Block {
        id: "sharpen",
        desc: "Unsharp mask: add the input's 5-tap high-pass detail, scaled by amount.",
        params: &[Param {
            name: "amount",
            default: 0.5,
            arity: 1,
            runtime: false,
        }],
        helper: "fn pfx_sharpen(c: vec3<f32>, uv: vec2<f32>, amount: f32) -> vec3<f32> {\n    let t = source_texel();\n    let ring = source_tap(uv + vec2<f32>(t.x, 0.0)) + source_tap(uv - vec2<f32>(t.x, 0.0)) + source_tap(uv + vec2<f32>(0.0, t.y)) + source_tap(uv - vec2<f32>(0.0, t.y));\n    let detail = source_tap(uv) - ring * 0.25;\n    return max(c + detail * amount, vec3<f32>(0.0));\n}",
        call: "pfx_sharpen({prev}, uv{args})",
        textures: &[],
    },
    Block {
        id: "radial_blur",
        desc: "Blur toward a centre point (speed / impact); strength is the streak length.",
        params: &[
            Param {
                name: "strength",
                default: 0.05,
                arity: 1,
            runtime: false,
            },
            Param {
                name: "center",
                default: 0.5,
                arity: 2,
            runtime: false,
            },
        ],
        helper: "fn pfx_radial_blur(c: vec3<f32>, uv: vec2<f32>, strength: f32, center: vec2<f32>) -> vec3<f32> {\n    let dir = (center - uv) * strength / 8.0;\n    var sum = vec3<f32>(0.0);\n    for (var i = 0; i < 8; i = i + 1) {\n        sum = sum + source_tap(uv + dir * f32(i));\n    }\n    return max(c + sum / 8.0 - source_tap(uv), vec3<f32>(0.0));\n}",
        call: "pfx_radial_blur({prev}, uv{args})",
        textures: &[],
    },
];
