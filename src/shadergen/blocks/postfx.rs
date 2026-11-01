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
//! like the engine's composite (`render::postfx::effects::assemble`).
//!
//! Most effects are per-pixel color grades that need only the sampled color + uv,
//! plus `game_time()` (#398) for animation (tint, scanline, grayscale, posterize,
//! damage vignette). **Sampling blocks** (#402: sharpen, radial blur) also tap the module's input at offsets through
//! the scaffold's `source_tap(uv)` / `source_texel()`. They read the module's
//! *input* — the colour the chain handed this module — not the running colour of
//! the blocks before them, and add the offset taps' **difference** from the
//! centre tap to the running colour. As the first block that is exactly the
//! re-sampled effect; later in the chain it layers the same detail over
//! whatever the earlier blocks graded. No new bindings, no new render targets.
//!
//! The chain runs authored modules **after tonemapping** (#397), on display-referred
//! colour in `[0, 1]`. Exposure, saturation and contrast were blocks here too; they
//! were removed because the volume already owns those knobs (`Graphics.SetExposure`
//! and friends, applied in HDR where they belong), and so were vignette, film grain
//! and chromatic aberration when the volume took those over (#1439,
//! `Graphics.SetVignette*` and friends); this catalog holds only looks the built-in
//! post stack can't make. `damage_vignette` stays: it is a gameplay overlay (a
//! tinted hurt edge a script drives), not the lens vignette.
//!
//! **CRT blocks** (#1679): `barrel` is the family's one **uv-stage** block, a warp of
//! where the frame is sampled (Unity's Lens Distortion at a positive intensity, the
//! glass's curve): every block after it, and the frame itself, reads the warped
//! image, and what falls past the curved screen draws black (`assemble::postfx`).
//! Its helper is `fn uv_barrel(uv, amount, corner_radius) -> vec2<f32>`, sending a
//! point outside the rounded screen to an off-frame sentinel. `phosphor_mask` is a
//! colour block in **screen pixels**: its call passes the fragment position
//! `in.pos.xy` instead of `uv`, so the RGB stripes are whole pixels at any
//! resolution, and it multiplies by a gain that keeps each channel's mean (a stripe
//! lit at 3×, the others at 0, mixed in by `strength`). Its `kind` is a choice param
//! (`Param::choice`), baked as an index.
//!
//! Params marked `runtime` (the strengths and the damage colour, #671) are read from
//! the module's param uniform instead of a baked `const`, so a script drives them
//! per frame on the volume with `Graphics.SetPostParam`; structural params (counts,
//! radii, levels, pulse speed) stay baked.

use super::{Block, Param, Reflection, Stage};

/// The postfx block catalog.
pub const BLOCKS: &[Block] = &[
    Block {
        id: "tint",
        desc: "Multiply the color by an RGB tint.",
        params: &[Param::baked("color", 1.0, 3)],
        helper: "fn pfx_tint(c: vec3<f32>, uv: vec2<f32>, color: vec3<f32>) -> vec3<f32> {\n    return c * color;\n}",
        call: "pfx_tint({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "grayscale",
        desc: "Collapse to Rec.709 luminance.",
        params: &[],
        helper: "fn pfx_grayscale(c: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return vec3<f32>(l);\n}",
        call: "pfx_grayscale({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "scanline",
        desc: "CRT-style horizontal scanlines; count and strength tune the look.",
        params: &[
            Param::baked("count", 240.0, 1),
            Param::live("strength", 0.2, 1),
        ],
        helper: "fn pfx_scanline(c: vec3<f32>, uv: vec2<f32>, count: f32, strength: f32) -> vec3<f32> {\n    let s = sin(uv.y * count * 3.14159265);\n    let line = 1.0 - strength * (0.5 - 0.5 * s);\n    return c * line;\n}",
        call: "pfx_scanline({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "posterize",
        desc: "Quantize the color into N bands per channel (retro / cel look).",
        params: &[Param::baked("levels", 6.0, 1)],
        helper: "fn pfx_posterize(c: vec3<f32>, uv: vec2<f32>, levels: f32) -> vec3<f32> {\n    let n = max(levels, 1.0);\n    return floor(c * n + 0.5) / n;\n}",
        call: "pfx_posterize({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "damage_vignette",
        desc: "Edge vignette blended toward a color (set it, e.g. red); pulses at pulse_speed.",
        params: &[
            Param::live("color", 1.0, 3),
            Param::live("intensity", 0.5, 1),
            Param::baked("pulse_speed", 0.0, 1),
        ],
        helper: "fn pfx_damage_vignette(c: vec3<f32>, uv: vec2<f32>, color: vec3<f32>, intensity: f32, pulse_speed: f32) -> vec3<f32> {\n    let edge = smoothstep(0.25, 0.75, distance(uv, vec2<f32>(0.5)));\n    let pulse = 0.75 + 0.25 * cos(game_time() * pulse_speed);\n    return mix(c, color, clamp(edge * intensity * pulse, 0.0, 1.0));\n}",
        call: "pfx_damage_vignette({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "sharpen",
        desc: "Unsharp mask: add the input's 5-tap high-pass detail, scaled by amount.",
        params: &[Param::live("amount", 0.5, 1)],
        helper: "fn pfx_sharpen(c: vec3<f32>, uv: vec2<f32>, amount: f32) -> vec3<f32> {\n    let t = source_texel();\n    let ring = source_tap(uv + vec2<f32>(t.x, 0.0)) + source_tap(uv - vec2<f32>(t.x, 0.0)) + source_tap(uv + vec2<f32>(0.0, t.y)) + source_tap(uv - vec2<f32>(0.0, t.y));\n    let detail = source_tap(uv) - ring * 0.25;\n    return max(c + detail * amount, vec3<f32>(0.0));\n}",
        call: "pfx_sharpen({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "radial_blur",
        desc: "Blur toward a centre point (speed / impact); strength is the streak length.",
        params: &[
            Param::live("strength", 0.05, 1),
            Param::baked("center", 0.5, 2),
        ],
        helper: "fn pfx_radial_blur(c: vec3<f32>, uv: vec2<f32>, strength: f32, center: vec2<f32>) -> vec3<f32> {\n    let dir = (center - uv) * strength / 8.0;\n    var sum = vec3<f32>(0.0);\n    for (var i = 0; i < 8; i = i + 1) {\n        sum = sum + source_tap(uv + dir * f32(i));\n    }\n    return max(c + sum / 8.0 - source_tap(uv), vec3<f32>(0.0));\n}",
        call: "pfx_radial_blur({prev}, uv{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },    Block {
        id: "barrel",
        desc: "CRT glass curvature: warp where the frame is sampled outward by amount; black past the curved screen, rounded corners.",
        params: &[
            Param::live("amount", 0.08, 1),
            Param::baked("corner_radius", 0.03, 1),
        ],
        helper: "fn uv_barrel(uv: vec2<f32>, amount: f32, corner_radius: f32) -> vec2<f32> {\n    let c = uv * 2.0 - 1.0;\n    let warped = c * (1.0 + amount * dot(c, c)) * 0.5 + 0.5;\n    let r = clamp(corner_radius, 0.0, 0.5);\n    let q = abs(warped - 0.5) - vec2<f32>(0.5 - r);\n    let outside = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;\n    return select(warped, vec2<f32>(-1.0), outside > 0.0);\n}",
        call: "uv_barrel({prev}{args})",
        textures: &[],
        stage: Stage::Uv,
        cut: None,
        reflection: Reflection::Kept,
    },
    Block {
        id: "phosphor_mask",
        desc: "CRT RGB phosphor mask in screen pixels (kind aperture | shadow | slot), brightness-compensated so the mean holds.",
        params: &[
            Param::choice("kind", &["aperture", "shadow", "slot"]),
            Param::baked("scale", 1.0, 1),
            Param::live("strength", 0.3, 1),
        ],
        helper: "fn pfx_phosphor_mask(c: vec3<f32>, px: vec2<f32>, kind: f32, scale: f32, strength: f32) -> vec3<f32> {\n    let q = vec2<i32>(floor(px / max(scale, 0.001)));\n    let shadow = kind > 0.5 && kind < 1.5;\n    let slot = kind > 1.5;\n    let lane = (q.x + select(0, q.y, shadow)) % 3;\n    let gap = slot && (q.y + 2 * (q.x / 3)) % 4 == 0;\n    let gain = select(select(3.0, 4.0, slot), 0.0, gap);\n    let mask = select(vec3<f32>(0.0), vec3<f32>(gain), vec3<i32>(0, 1, 2) == vec3<i32>(lane));\n    return c * mix(vec3<f32>(1.0), mask, strength);\n}",
        call: "pfx_phosphor_mask({prev}, in.pos.xy{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
        reflection: Reflection::Kept,
    },
];
