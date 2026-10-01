//! src/shadergen/blocks/surface.rs — the surface building-block family (#272).
//!
//! Surface blocks vary the **fragment look** of the forward pass while keeping the
//! standard `vs_main` + the full PBR lighting verbatim (so the assembled module
//! binds against the forward pipeline unchanged — same `VertexInput`, same
//! `LightingUniforms`/`EntityUniforms`/group(2) maps, same `vs_main`/`fs_main`).
//! Each block's `helper` is a pure function
//! `fn srf_<id>(c: vec3<f32>, in: VertexOutput, <params…>) -> vec3<f32>` that
//! transforms the lit color; the assembler folds the chosen blocks (in recipe order) after the
//! standard lighting and before the final write, so they restyle the shaded
//! result rather than replace the lighting model.
//!
//! Blocks read only what the forward contract already exposes — the interpolated
//! `VertexOutput` (`world_position`/`world_normal`/`tex_coords`) and the `camera`
//! and `entity` uniforms. Animated blocks read `camera.time`, the sim's game time
//! (#398): it freezes with the game and is 0 in edit mode. The one binding they
//! add is the material's param uniform (#399): a param marked `runtime` is read from
//! it, so gameplay drives it (`hit_flash.amount` from a script) with no re-bake.
//! A block that needs a pattern samples an extra texture slot (#400): `t_mask`, the
//! texture the material names under `mask` (white when it names none), through the
//! material's sampler `s_diffuse`.
//!
//! A [`Stage::Uv`] block (`uv_scroll`, #401) is the exception to "transform the lit
//! color": it rewrites `in.tex_coords` at the top of `fs_main`, before any map is
//! sampled, so every lookup — and every later color block reading `in.tex_coords` —
//! sees the moved UVs.

use super::{Block, Param, Stage};

/// The surface block catalog.
pub const BLOCKS: &[Block] = &[
    Block {
        id: "toon_ramp",
        desc: "Quantize the lit color into N flat bands (cel shading).",
        params: &[Param::baked("steps", 4.0, 1)],
        helper: "fn srf_toon_ramp(c: vec3<f32>, in: VertexOutput, steps: f32) -> vec3<f32> {\n    let n = max(steps, 1.0);\n    return floor(c * n + 0.5) / n;\n}",
        call: "srf_toon_ramp({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "fresnel_rim",
        desc: "Add a view-dependent rim glow (Fresnel) in a chosen color.",
        params: &[
            Param::live("color", 1.0, 3),
            Param::baked("power", 3.0, 1),
            Param::live("strength", 1.0, 1),
        ],
        helper: "fn srf_fresnel_rim(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, power: f32, strength: f32) -> vec3<f32> {\n    let n = normalize(in.world_normal);\n    let v = normalize(camera.camera_pos - in.world_position);\n    let rim = pow(1.0 - max(dot(n, v), 0.0), power);\n    return c + color * (rim * strength);\n}",
        call: "srf_fresnel_rim({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "tint",
        desc: "Multiply the lit color by an RGB tint.",
        params: &[Param::live("color", 1.0, 3)],
        helper: "fn srf_tint(c: vec3<f32>, in: VertexOutput, color: vec3<f32>) -> vec3<f32> {\n    return c * color;\n}",
        call: "srf_tint({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "emissive_boost",
        desc: "Add a flat emissive glow scaled by the diffuse map's luminance (mask).",
        params: &[
            Param::live("color", 1.0, 3),
            Param::live("strength", 1.0, 1),
        ],
        helper: "fn srf_emissive_boost(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, strength: f32) -> vec3<f32> {\n    let mask = textureSample(t_emissive, s_diffuse, in.tex_coords).rgb;\n    return c + color * mask * strength;\n}",
        call: "srf_emissive_boost({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "uv_scroll_stripes",
        desc: "Modulate brightness with stripes that scroll along V over game time (scanlines, energy fields).",
        params: &[
            Param::baked("frequency", 16.0, 1),
            Param::baked("strength", 0.25, 1),
            Param::baked("speed", 1.0, 1),
        ],
        helper: "fn srf_uv_scroll_stripes(c: vec3<f32>, in: VertexOutput, frequency: f32, strength: f32, speed: f32) -> vec3<f32> {\n    let s = sin((in.tex_coords.y * frequency + camera.time * speed) * 6.2831853);\n    let band = 1.0 - strength * (0.5 - 0.5 * s);\n    return c * band;\n}",
        call: "srf_uv_scroll_stripes({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "desaturate",
        desc: "Pull the lit color toward grayscale (0 = full color, 1 = gray).",
        params: &[Param::live("amount", 0.5, 1)],
        helper: "fn srf_desaturate(c: vec3<f32>, in: VertexOutput, amount: f32) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return mix(c, vec3<f32>(l), amount);\n}",
        call: "srf_desaturate({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "height_fog",
        desc: "Per-material look: blend toward a color as world height drops below a line (scene fog is separate, applied after).",
        params: &[
            Param::baked("color", 0.5, 3),
            Param::baked("top", 5.0, 1),
            Param::baked("bottom", 0.0, 1),
        ],
        helper: "fn srf_height_fog(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, top: f32, bottom: f32) -> vec3<f32> {\n    let t = clamp((top - in.world_position.y) / max(top - bottom, 0.001), 0.0, 1.0);\n    return mix(c, color, t);\n}",
        call: "srf_height_fog({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "hit_flash",
        desc: "Blend the lit color toward a flash color by `amount` (0 = off, 1 = solid) — hit feedback, driven at runtime.",
        params: &[
            Param::live("color", 1.0, 3),
            Param::live("amount", 0.0, 1),
        ],
        helper: "fn srf_hit_flash(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, amount: f32) -> vec3<f32> {\n    return mix(c, color, clamp(amount, 0.0, 1.0));\n}",
        call: "srf_hit_flash({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "dissolve",
        desc: "Burn the surface away where the `mask` texture is below `amount` (0 = whole, 1 = gone), with a glowing edge — dissolve-on-death, driven at runtime.",
        params: &[
            Param::live("amount", 0.0, 1),
            Param::baked("edge_width", 0.05, 1),
            Param::live("edge_color", 1.0, 3),
            Param::baked("tiling", 1.0, 1),
        ],
        helper: "fn srf_dissolve_cut(in: VertexOutput, amount: f32, edge_width: f32, edge_color: vec3<f32>, tiling: f32) -> f32 {\n    let m = textureSample(t_mask, s_diffuse, in.tex_coords * tiling).r;\n    if (amount > 0.0 && m < amount) {\n        discard;\n    }\n    return m;\n}\nfn srf_dissolve(c: vec3<f32>, in: VertexOutput, amount: f32, edge_width: f32, edge_color: vec3<f32>, tiling: f32) -> vec3<f32> {\n    let m = srf_dissolve_cut(in, amount, edge_width, edge_color, tiling);\n    let edge = 1.0 - smoothstep(amount, amount + max(edge_width, 0.0001), m);\n    return mix(c, edge_color, select(0.0, edge, amount > 0.0));\n}",
        call: "srf_dissolve({prev}, in{args})",
        textures: &["mask"],
        stage: Stage::Color,
        cut: Some("srf_dissolve_cut(in{args})"),
    },
    Block {
        id: "detail_overlay",
        desc: "Overlay the `mask` texture tiled across the surface (grime, scratches, a scrolling energy pattern when `scroll` is set): 0.5 is neutral, darker darkens, lighter brightens.",
        params: &[
            Param::baked("tiling", 4.0, 1),
            Param::live("strength", 1.0, 1),
            Param::baked("scroll", 0.0, 2),
        ],
        helper: "fn srf_detail_overlay(c: vec3<f32>, in: VertexOutput, tiling: f32, strength: f32, scroll: vec2<f32>) -> vec3<f32> {\n    let uv = in.tex_coords * tiling + scroll * camera.time;\n    let d = textureSample(t_mask, s_diffuse, uv).rgb;\n    return c * mix(vec3<f32>(1.0), d * 2.0, strength);\n}",
        call: "srf_detail_overlay({prev}, in{args})",
        textures: &["mask"],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "pulse_glow",
        desc: "Add an emissive glow that breathes over game time (pickups, objectives): `speed` pulses per game second, peak `strength`.",
        params: &[
            Param::live("color", 1.0, 3),
            Param::baked("speed", 1.0, 1),
            Param::live("strength", 1.0, 1),
        ],
        helper: "fn srf_pulse_glow(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, speed: f32, strength: f32) -> vec3<f32> {\n    let p = 0.5 + 0.5 * sin(camera.time * speed * 6.2831853);\n    return c + color * (strength * p);\n}",
        call: "srf_pulse_glow({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "uv_scroll",
        desc: "Offset the surface's UVs by `speed` uv per game second before any map is sampled (conveyors, flowing energy, water).",
        params: &[Param::baked("speed", 0.25, 2)],
        helper: "fn uv_uv_scroll(uv: vec2<f32>, in: VertexOutput, speed: vec2<f32>) -> vec2<f32> {\n    return uv + speed * camera.time;\n}",
        call: "uv_uv_scroll({prev}, in{args})",
        textures: &[],
        stage: Stage::Uv,
        cut: None,
    },
    Block {
        id: "triplanar_detail",
        desc: "Overlay the `mask` texture projected in world space from three axes, blended by the normal (no UV stretching on level geometry): `scale` tiles per world unit, 0.5 gray is neutral.",
        params: &[
            Param::baked("scale", 0.5, 1),
            Param::live("strength", 1.0, 1),
        ],
        helper: "fn srf_triplanar_detail(c: vec3<f32>, in: VertexOutput, scale: f32, strength: f32) -> vec3<f32> {\n    let a = pow(abs(normalize(in.world_normal)), vec3<f32>(4.0));\n    let w = a / max(a.x + a.y + a.z, 0.0001);\n    let p = in.world_position * scale;\n    let x = textureSample(t_mask, s_diffuse, p.zy).rgb;\n    let y = textureSample(t_mask, s_diffuse, p.xz).rgb;\n    let z = textureSample(t_mask, s_diffuse, p.xy).rgb;\n    let d = x * w.x + y * w.y + z * w.z;\n    return c * mix(vec3<f32>(1.0), d * 2.0, strength);\n}",
        call: "srf_triplanar_detail({prev}, in{args})",
        textures: &["mask"],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "hologram",
        desc: "Hologram look for in-world UI and shields: world-height scanlines rolling over game time, a Fresnel rim and a stepped flicker, in `color` over a faint copy of the surface.",
        params: &[
            Param::live("color", 1.0, 3),
            Param::baked("line_freq", 20.0, 1),
            Param::baked("flicker", 0.15, 1),
        ],
        helper: "fn srf_hologram(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, line_freq: f32, flicker: f32) -> vec3<f32> {\n    let n = normalize(in.world_normal);\n    let v = normalize(camera.camera_pos - in.world_position);\n    let rim = pow(1.0 - max(dot(n, v), 0.0), 2.0);\n    let lines = 0.5 + 0.5 * sin((in.world_position.y * line_freq - camera.time * 2.0) * 6.2831853);\n    let h = fract(sin(floor(camera.time * 24.0) * 12.9898) * 43758.5453);\n    let f = 1.0 - clamp(flicker, 0.0, 1.0) * h;\n    return (c * 0.25 + color * (0.35 + rim + 0.5 * lines)) * f;\n}",
        call: "srf_hologram({prev}, in{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
];
