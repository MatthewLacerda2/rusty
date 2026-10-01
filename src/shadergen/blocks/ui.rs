//! src/shadergen/blocks/ui.rs — the ui building-block family (#427).
//!
//! UI blocks restyle **one graphic** — an `Image`, `Shape` or `Text` that names the baked
//! shader. Each helper is a pure function
//! `fn ui_<id>(c: vec4<f32>, in: VertexOut, f: UiFrag, <params…>) -> vec4<f32>`:
//! `c` is the graphic's colour so far, **premultiplied, display space** (what the
//! UI pass blends), `in` the interpolated vertex and `f` the per-graphic inputs the
//! assembler's scaffold computes (`assemble::ui`): `f.uv` rect-local (0,0 bottom-left
//! to 1,1 top-right), `f.size` the rect in pixels, `f.pixel` the fragment's pixel,
//! `f.time` the UI clock.
//!
//! Sampling blocks (`rgb_split`, `glitch_slices`) re-shade the graphic at a
//! rect-local offset through the scaffold's `ui_shift`, which — like the postfx
//! sampling blocks (#402) — adds the tap's difference from the centre to the running
//! colour, and draws nothing where the offset leaves the rect. Helpers call it
//! unconditionally: it samples a texture, which must stay in uniform control flow.
//!
//! Premultiplied output may carry more colour than alpha: that light **adds** to
//! what is under the graphic (the hologram's glow does this on purpose).

use super::{Block, Param, Stage};

/// The ui block catalog.
pub const BLOCKS: &[Block] = &[
    Block {
        id: "scanlines",
        desc: "Horizontal scanlines `spacing` px apart that fade the graphic; `speed` scrolls them (px/s).",
        params: &[
            Param::baked("spacing", 4.0, 1),
            Param::live("strength", 0.35, 1),
            Param::baked("speed", 0.0, 1),
        ],
        helper: "fn ui_scanlines(c: vec4<f32>, in: VertexOut, f: UiFrag, spacing: f32, strength: f32, speed: f32) -> vec4<f32> {\n    let y = f.uv.y * f.size.y + f.time * speed;\n    let line = 0.5 + 0.5 * cos(y / max(spacing, 1.0) * 6.2831853);\n    return c * (1.0 - clamp(strength, 0.0, 1.0) * line);\n}",
        call: "ui_scanlines({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "rgb_split",
        desc: "Chromatic split: red and blue re-sampled `offset` px apart along `angle` degrees.",
        params: &[Param::live("offset", 3.0, 1), Param::baked("angle", 0.0, 1)],
        helper: "fn ui_rgb_split(c: vec4<f32>, in: VertexOut, f: UiFrag, offset: f32, angle: f32) -> vec4<f32> {\n    let a = radians(angle);\n    let d = vec2<f32>(cos(a), sin(a)) * offset / max(f.size, vec2<f32>(1.0));\n    let r = ui_shift(c, in, f, -d);\n    let b = ui_shift(c, in, f, d);\n    return vec4<f32>(r.r, c.g, b.b, max(c.a, max(r.a, b.a)));\n}",
        call: "ui_rgb_split({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "glitch_slices",
        desc: "Horizontal bands jump sideways up to `amount` px; `density` of the `bands` glitch, re-rolled `rate` times a second.",
        params: &[
            Param::baked("bands", 8.0, 1),
            Param::live("amount", 12.0, 1),
            Param::baked("density", 0.35, 1),
            Param::baked("rate", 12.0, 1),
        ],
        helper: "fn ui_glitch_slices(c: vec4<f32>, in: VertexOut, f: UiFrag, bands: f32, amount: f32, density: f32, rate: f32) -> vec4<f32> {\n    let band = floor(f.uv.y * max(bands, 1.0));\n    let tick = floor(f.time * rate);\n    let on = step(ui_hash(vec2<f32>(band, tick)), density);\n    let shift = (ui_hash(vec2<f32>(band + 17.0, tick)) * 2.0 - 1.0) * amount * on;\n    return ui_shift(c, in, f, vec2<f32>(-shift / max(f.size.x, 1.0), 0.0));\n}",
        call: "ui_glitch_slices({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "noise_flicker",
        desc: "The whole graphic flickers dimmer by up to `strength`, re-rolled `rate` times a second.",
        params: &[Param::live("strength", 0.35, 1), Param::baked("rate", 24.0, 1)],
        helper: "fn ui_noise_flicker(c: vec4<f32>, in: VertexOut, f: UiFrag, strength: f32, rate: f32) -> vec4<f32> {\n    let n = ui_hash(vec2<f32>(floor(f.time * rate), 3.1));\n    return c * (1.0 - clamp(strength, 0.0, 1.0) * n);\n}",
        call: "ui_noise_flicker({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "hologram",
        desc: "Hologram: luminance recoloured to `hue` degrees, glowing additively, with scrolling scan bands and a flicker.",
        params: &[
            Param::baked("hue", 190.0, 1),
            Param::live("strength", 1.0, 1),
            Param::baked("spacing", 3.0, 1),
            Param::baked("speed", 30.0, 1),
            Param::baked("flicker", 0.12, 1),
        ],
        helper: "fn ui_hologram(c: vec4<f32>, in: VertexOut, f: UiFrag, hue: f32, strength: f32, spacing: f32, speed: f32, flicker: f32) -> vec4<f32> {\n    let h = fract(hue / 360.0 + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0));\n    let tint = mix(clamp(abs(h * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0), 0.25);\n    let lum = dot(c.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));\n    let y = f.uv.y * f.size.y - f.time * speed;\n    let scan = 0.7 + 0.3 * cos(y / max(spacing, 1.0) * 6.2831853);\n    let flick = 1.0 - flicker * ui_hash(vec2<f32>(floor(f.time * 30.0), 7.0));\n    let holo = vec4<f32>(tint * (lum + 0.35 * c.a), 0.75 * c.a) * (scan * flick);\n    return mix(c, holo, clamp(strength, 0.0, 1.0));\n}",
        call: "ui_hologram({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "dissolve",
        desc: "Noise dissolve: `amount` 0 shows all, 1 nothing; the burning edge (`edge` wide) glows `edge_color`.",
        params: &[
            Param::live("amount", 0.0, 1),
            Param::baked("scale", 12.0, 1),
            Param::baked("edge", 0.08, 1),
            Param::baked("edge_color", 1.0, 3),
        ],
        helper: "fn ui_dissolve(c: vec4<f32>, in: VertexOut, f: UiFrag, amount: f32, scale: f32, edge: f32, edge_color: vec3<f32>) -> vec4<f32> {\n    let aspect = f.size.x / max(f.size.y, 1.0);\n    let n = ui_noise(f.uv * vec2<f32>(aspect, 1.0) * scale);\n    let t = clamp(amount, 0.0, 1.0) * (1.0 + edge);\n    let keep = step(t, n);\n    let glow = (1.0 - smoothstep(t, t + max(edge, 1e-4), n)) * step(1e-4, t);\n    return vec4<f32>(mix(c.rgb, edge_color * c.a, glow), c.a) * keep;\n}",
        call: "ui_dissolve({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "wipe",
        desc: "Directional reveal: `progress` 0 hides, 1 shows all, sweeping toward `angle` degrees (0: left to right) with a `softness` edge.",
        params: &[
            Param::live("progress", 1.0, 1),
            Param::baked("angle", 0.0, 1),
            Param::baked("softness", 0.05, 1),
        ],
        helper: "fn ui_wipe(c: vec4<f32>, in: VertexOut, f: UiFrag, progress: f32, angle: f32, softness: f32) -> vec4<f32> {\n    let a = radians(angle);\n    let dir = vec2<f32>(cos(a), sin(a));\n    let reach = abs(dir.x) + abs(dir.y);\n    let t = dot(f.uv - vec2<f32>(0.5), dir) / max(reach, 1e-4) + 0.5;\n    return c * ui_reveal(t, progress, softness);\n}",
        call: "ui_wipe({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
    Block {
        id: "radial_wipe",
        desc: "Clock-sweep reveal around the rect's centre: `progress` 0 hides, 1 shows all, from `start` degrees (90: twelve o'clock), clockwise unless `clockwise` is 0.",
        params: &[
            Param::live("progress", 1.0, 1),
            Param::baked("start", 90.0, 1),
            Param::baked("clockwise", 1.0, 1),
            Param::baked("softness", 0.01, 1),
        ],
        helper: "fn ui_radial_wipe(c: vec4<f32>, in: VertexOut, f: UiFrag, progress: f32, start: f32, clockwise: f32, softness: f32) -> vec4<f32> {\n    let p = (f.uv - vec2<f32>(0.5)) * f.size;\n    let ang = degrees(atan2(p.y, p.x));\n    let t = fract(select(ang - start, start - ang, clockwise > 0.5) / 360.0);\n    return c * ui_reveal(t, progress, softness);\n}",
        call: "ui_radial_wipe({prev}, in, f{args})",
        textures: &[],
        stage: Stage::Color,
        cut: None,
    },
];
