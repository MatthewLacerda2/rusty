//! src/shadergen/blocks/surface.rs — the surface building-block family (#272).
//!
//! Surface blocks vary the **fragment look** of the forward pass while keeping the
//! standard `vs_main` + the full PBR lighting verbatim (so the assembled module
//! binds against the forward pipeline unchanged — same `VertexInput`, same
//! `LightingUniforms`/`EntityUniforms`/group(2) maps, same `vs_main`/`fs_main`).
//! Each block's `helper` is a pure function
//! `fn srf_<id>(c: vec3<f32>, in: VertexOutput) -> vec3<f32>` that transforms the
//! lit color; the assembler folds the chosen blocks (in recipe order) after the
//! standard lighting and before the final write, so they restyle the shaded
//! result rather than replace the lighting model.
//!
//! Blocks read only what the forward contract already exposes — the interpolated
//! `VertexOutput` (`world_position`/`world_normal`/`tex_coords`), the `camera` and
//! `entity` uniforms, and `Time` is not available in the sim-free render path, so
//! "animation" blocks drive off UV/position, not wall-clock. No new bindings.

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
        }],
        helper: "fn srf_toon_ramp(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    let n = max(toon_ramp_steps, 1.0);\n    return floor(c * n + 0.5) / n;\n}",
        call: "srf_toon_ramp({prev}, in)",
    },
    Block {
        id: "fresnel_rim",
        desc: "Add a view-dependent rim glow (Fresnel) in a chosen color.",
        params: &[
            Param {
                name: "color",
                default: 1.0,
                arity: 3,
            },
            Param {
                name: "power",
                default: 3.0,
                arity: 1,
            },
            Param {
                name: "strength",
                default: 1.0,
                arity: 1,
            },
        ],
        helper: "fn srf_fresnel_rim(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    let n = normalize(in.world_normal);\n    let v = normalize(camera.camera_pos - in.world_position);\n    let rim = pow(1.0 - max(dot(n, v), 0.0), fresnel_rim_power);\n    return c + fresnel_rim_color * (rim * fresnel_rim_strength);\n}",
        call: "srf_fresnel_rim({prev}, in)",
    },
    Block {
        id: "tint",
        desc: "Multiply the lit color by an RGB tint.",
        params: &[Param {
            name: "color",
            default: 1.0,
            arity: 3,
        }],
        helper: "fn srf_tint(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    return c * tint_color;\n}",
        call: "srf_tint({prev}, in)",
    },
    Block {
        id: "emissive_boost",
        desc: "Add a flat emissive glow scaled by the diffuse map's luminance (mask).",
        params: &[
            Param {
                name: "color",
                default: 1.0,
                arity: 3,
            },
            Param {
                name: "strength",
                default: 1.0,
                arity: 1,
            },
        ],
        helper: "fn srf_emissive_boost(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    let mask = textureSample(t_emissive, s_diffuse, in.tex_coords).rgb;\n    return c + emissive_boost_color * mask * emissive_boost_strength;\n}",
        call: "srf_emissive_boost({prev}, in)",
    },
    Block {
        id: "uv_scroll_stripes",
        desc: "Modulate brightness with UV-scrolled stripes (a static scanline-like band over the surface).",
        params: &[
            Param {
                name: "frequency",
                default: 16.0,
                arity: 1,
            },
            Param {
                name: "strength",
                default: 0.25,
                arity: 1,
            },
        ],
        helper: "fn srf_uv_scroll_stripes(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    let s = sin(in.tex_coords.y * uv_scroll_stripes_frequency * 6.2831853);\n    let band = 1.0 - uv_scroll_stripes_strength * (0.5 - 0.5 * s);\n    return c * band;\n}",
        call: "srf_uv_scroll_stripes({prev}, in)",
    },
    Block {
        id: "desaturate",
        desc: "Pull the lit color toward grayscale (0 = full color, 1 = gray).",
        params: &[Param {
            name: "amount",
            default: 0.5,
            arity: 1,
        }],
        helper: "fn srf_desaturate(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));\n    return mix(c, vec3<f32>(l), desaturate_amount);\n}",
        call: "srf_desaturate({prev}, in)",
    },
    Block {
        id: "height_fog",
        desc: "Blend toward a fog color as world-space height drops below a line.",
        params: &[
            Param {
                name: "color",
                default: 0.5,
                arity: 3,
            },
            Param {
                name: "top",
                default: 5.0,
                arity: 1,
            },
            Param {
                name: "bottom",
                default: 0.0,
                arity: 1,
            },
        ],
        helper: "fn srf_height_fog(c: vec3<f32>, in: VertexOutput) -> vec3<f32> {\n    let t = clamp((height_fog_top - in.world_position.y) / max(height_fog_top - height_fog_bottom, 0.001), 0.0, 1.0);\n    return mix(c, height_fog_color, t);\n}",
        call: "srf_height_fog({prev}, in)",
    },
];
