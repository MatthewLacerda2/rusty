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

use super::{Block, Param};

/// The surface block catalog.
pub const BLOCKS: &[Block] = &[
    Block {
        id: "toon_ramp",
        desc: "Quantize the lit color into N flat bands (cel shading).",
        params: &[Param {
            name: "steps",
            default: 4.0,
            arity: 1,
            runtime: false,
        }],
        helper: "fn srf_toon_ramp(c: vec3<f32>, in: VertexOutput, steps: f32) -> vec3<f32> {\n    let n = max(steps, 1.0);\n    return floor(c * n + 0.5) / n;\n}",
        call: "srf_toon_ramp({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "fresnel_rim",
        desc: "Add a view-dependent rim glow (Fresnel) in a chosen color.",
        params: &[
            Param {
                name: "color",
                default: 1.0,
                arity: 3,
                runtime: true,
            },
            Param {
                name: "power",
                default: 3.0,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "strength",
                default: 1.0,
                arity: 1,
                runtime: true,
            },
        ],
        helper: "fn srf_fresnel_rim(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, power: f32, strength: f32) -> vec3<f32> {\n    let n = normalize(in.world_normal);\n    let v = normalize(camera.camera_pos - in.world_position);\n    let rim = pow(1.0 - max(dot(n, v), 0.0), power);\n    return c + color * (rim * strength);\n}",
        call: "srf_fresnel_rim({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "tint",
        desc: "Multiply the lit color by an RGB tint.",
        params: &[Param {
            name: "color",
            default: 1.0,
            arity: 3,
            runtime: true,
        }],
        helper: "fn srf_tint(c: vec3<f32>, in: VertexOutput, color: vec3<f32>) -> vec3<f32> {\n    return c * color;\n}",
        call: "srf_tint({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "emissive_boost",
        desc: "Add a flat emissive glow scaled by the diffuse map's luminance (mask).",
        params: &[
            Param {
                name: "color",
                default: 1.0,
                arity: 3,
                runtime: true,
            },
            Param {
                name: "strength",
                default: 1.0,
                arity: 1,
                runtime: true,
            },
        ],
        helper: "fn srf_emissive_boost(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, strength: f32) -> vec3<f32> {\n    let mask = textureSample(t_emissive, s_diffuse, in.tex_coords).rgb;\n    return c + color * mask * strength;\n}",
        call: "srf_emissive_boost({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "uv_scroll_stripes",
        desc: "Modulate brightness with stripes that scroll along V over game time (scanlines, energy fields).",
        params: &[
            Param {
                name: "frequency",
                default: 16.0,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "strength",
                default: 0.25,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "speed",
                default: 1.0,
                arity: 1,
                runtime: false,
            },
        ],
        helper: "fn srf_uv_scroll_stripes(c: vec3<f32>, in: VertexOutput, frequency: f32, strength: f32, speed: f32) -> vec3<f32> {\n    let s = sin((in.tex_coords.y * frequency + camera.time * speed) * 6.2831853);\n    let band = 1.0 - strength * (0.5 - 0.5 * s);\n    return c * band;\n}",
        call: "srf_uv_scroll_stripes({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "desaturate",
        desc: "Pull the lit color toward grayscale (0 = full color, 1 = gray).",
        params: &[Param {
            name: "amount",
            default: 0.5,
            arity: 1,
            runtime: true,
        }],
        helper: "fn srf_desaturate(c: vec3<f32>, in: VertexOutput, amount: f32) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return mix(c, vec3<f32>(l), amount);\n}",
        call: "srf_desaturate({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "height_fog",
        desc: "Per-material look: blend toward a color as world height drops below a line (scene fog is separate, applied after).",
        params: &[
            Param {
                name: "color",
                default: 0.5,
                arity: 3,
                runtime: false,
            },
            Param {
                name: "top",
                default: 5.0,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "bottom",
                default: 0.0,
                arity: 1,
                runtime: false,
            },
        ],
        helper: "fn srf_height_fog(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, top: f32, bottom: f32) -> vec3<f32> {\n    let t = clamp((top - in.world_position.y) / max(top - bottom, 0.001), 0.0, 1.0);\n    return mix(c, color, t);\n}",
        call: "srf_height_fog({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "hit_flash",
        desc: "Blend the lit color toward a flash color by `amount` (0 = off, 1 = solid) — hit feedback, driven at runtime.",
        params: &[
            Param {
                name: "color",
                default: 1.0,
                arity: 3,
                runtime: true,
            },
            Param {
                name: "amount",
                default: 0.0,
                arity: 1,
                runtime: true,
            },
        ],
        helper: "fn srf_hit_flash(c: vec3<f32>, in: VertexOutput, color: vec3<f32>, amount: f32) -> vec3<f32> {\n    return mix(c, color, clamp(amount, 0.0, 1.0));\n}",
        call: "srf_hit_flash({prev}, in{args})",
        textures: &[],
    },
    Block {
        id: "dissolve",
        desc: "Burn the surface away where the `mask` texture is below `amount` (0 = whole, 1 = gone), with a glowing edge — dissolve-on-death, driven at runtime.",
        params: &[
            Param {
                name: "amount",
                default: 0.0,
                arity: 1,
                runtime: true,
            },
            Param {
                name: "edge_width",
                default: 0.05,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "edge_color",
                default: 1.0,
                arity: 3,
                runtime: true,
            },
            Param {
                name: "tiling",
                default: 1.0,
                arity: 1,
                runtime: false,
            },
        ],
        helper: "fn srf_dissolve(c: vec3<f32>, in: VertexOutput, amount: f32, edge_width: f32, edge_color: vec3<f32>, tiling: f32) -> vec3<f32> {\n    let m = textureSample(t_mask, s_diffuse, in.tex_coords * tiling).r;\n    if (amount > 0.0 && m < amount) {\n        discard;\n    }\n    let edge = 1.0 - smoothstep(amount, amount + max(edge_width, 0.0001), m);\n    return mix(c, edge_color, select(0.0, edge, amount > 0.0));\n}",
        call: "srf_dissolve({prev}, in{args})",
        textures: &["mask"],
    },
    Block {
        id: "detail_overlay",
        desc: "Overlay the `mask` texture tiled across the surface (grime, scratches, a scrolling energy pattern when `scroll` is set): 0.5 is neutral, darker darkens, lighter brightens.",
        params: &[
            Param {
                name: "tiling",
                default: 4.0,
                arity: 1,
                runtime: false,
            },
            Param {
                name: "strength",
                default: 1.0,
                arity: 1,
                runtime: true,
            },
            Param {
                name: "scroll",
                default: 0.0,
                arity: 2,
                runtime: false,
            },
        ],
        helper: "fn srf_detail_overlay(c: vec3<f32>, in: VertexOutput, tiling: f32, strength: f32, scroll: vec2<f32>) -> vec3<f32> {\n    let uv = in.tex_coords * tiling + scroll * camera.time;\n    let d = textureSample(t_mask, s_diffuse, uv).rgb;\n    return c * mix(vec3<f32>(1.0), d * 2.0, strength);\n}",
        call: "srf_detail_overlay({prev}, in{args})",
        textures: &["mask"],
    },
];
