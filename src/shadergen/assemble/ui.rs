//! src/shadergen/assemble/ui.rs — a ui recipe → a UI shader variant (#427).
//!
//! The base is the engine's own `ui.wgsl`, kept verbatim — both entry-point pairs
//! (`vs_main`/`fs_main` for screen canvases, `vs_world`/`fs_world` for world ones),
//! the sRGB handling, the SDF text and the world pass's RectMask clip — so a variant
//! draws exactly like the standard UI shader, through the same vertex format and the
//! same group-0 texture binding. Only `graphic()`, the one function both fragment
//! entry points take a graphic's colour from, is replaced: the variant's version
//! folds the block chain over `shade(in)`.
//!
//! A variant adds one binding, group 3: the per-graphic `UiShade` uniform
//! ([`UI_UNIFORM_DECL`]) — the graphic's rect, the UI clock and the runtime-param
//! slots — and the scaffold below, which turns it into the `UiFrag` the blocks read.

use std::fmt::Write as _;

use super::emit::{chain, emit_helpers, emit_params, Instance};
use crate::shadergen::blocks::Stage;
use crate::shadergen::params::{ParamLayout, UI_UNIFORM_DECL};
use crate::shadergen::recipe::ShaderRecipe;

/// The function in `ui.wgsl` a variant replaces (the splice point).
pub(crate) const GRAPHIC: &str =
    "fn graphic(in: VertexOut) -> vec4<f32> {\n    return shade(in);\n}";

/// Assemble a ui variant by replacing `base`'s [`GRAPHIC`] with the folded chain.
pub(super) fn assemble(
    recipe: &ShaderRecipe,
    resolved: &[Instance],
    layout: &ParamLayout,
    base: &str,
) -> Result<String, String> {
    if !base.contains(GRAPHIC) {
        return Err("ui base shader is missing the expected `graphic` splice point".into());
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// {name}.wgsl — authored ui variant (#427), assembled from {n} block(s).\n\
         // Base: the UI pass (`ui.wgsl`) kept verbatim; `graphic()` folds the blocks\n\
         // over the graphic's premultiplied colour, on screen and world canvases.",
        name = recipe.name,
        n = resolved.len()
    );
    out.push_str(UI_UNIFORM_DECL);
    out.push_str(SCAFFOLD);
    out.push_str("\n// ---- authored ui blocks ----\n");
    emit_params(&mut out, resolved, layout);
    emit_helpers(&mut out, resolved);
    let folded = chain(resolved, "shade(in)", layout, Stage::Color);
    let graphic = format!(
        "fn graphic(in: VertexOut) -> vec4<f32> {{\n    let f = ui_frag(in);\n    return {folded};\n}}"
    );
    let _ = write!(out, "\n{}", base.replace(GRAPHIC, &graphic));
    Ok(out)
}

/// What every ui variant declares besides its blocks: the `UiFrag` inputs, built
/// from the `UiShade` uniform and the fragment, and the shared helpers.
///
/// `UiFrag.duv` maps a rect-uv offset to a texture-uv offset (from the screen
/// derivatives of both), so `ui_tap` can re-shade the graphic — image texel, SDF
/// glyph, or a Shape's field and a gradient through the rect-local `local` (#425) —
/// at an offset. A tap off the rect draws nothing.
const SCAFFOLD: &str = r#"
struct UiFrag {
    uv: vec2<f32>,
    size: vec2<f32>,
    pixel: vec2<f32>,
    time: f32,
    duv: mat2x2<f32>,
};

fn ui_inverse(m: mat2x2<f32>) -> mat2x2<f32> {
    let det = m[0].x * m[1].y - m[1].x * m[0].y;
    let inv = 1.0 / select(det, 1e-12, abs(det) < 1e-12);
    return mat2x2<f32>(vec2<f32>(m[1].y, -m[0].y), vec2<f32>(-m[1].x, m[0].x)) * inv;
}

fn ui_frag(in: VertexOut) -> UiFrag {
    let axes = mat2x2<f32>(shader_params.origin_x.zw, shader_params.axis_y_size.xy);
    var f: UiFrag;
    f.uv = ui_inverse(axes) * (in.canvas - shader_params.origin_x.xy);
    f.size = shader_params.axis_y_size.zw;
    f.pixel = in.clip.xy;
    f.time = shader_params.time.x;
    let rect = mat2x2<f32>(dpdx(f.uv), dpdy(f.uv));
    let tex = mat2x2<f32>(dpdx(in.uv), dpdy(in.uv));
    f.duv = tex * ui_inverse(rect);
    return f;
}

fn ui_tap(in: VertexOut, f: UiFrag, d: vec2<f32>) -> vec4<f32> {
    var t = in;
    t.uv = in.uv + f.duv * d;
    t.local = vec4<f32>(in.local.xy + d * in.local.zw * 2.0, in.local.zw);
    let p = f.uv + d;
    let inside = all(p >= vec2<f32>(0.0)) && all(p <= vec2<f32>(1.0));
    return shade(t) * select(0.0, 1.0, inside);
}

fn ui_shift(c: vec4<f32>, in: VertexOut, f: UiFrag, d: vec2<f32>) -> vec4<f32> {
    let s = clamp(c + ui_tap(in, f, d) - ui_tap(in, f, vec2<f32>(0.0)), vec4<f32>(0.0), vec4<f32>(1.0));
    return vec4<f32>(min(s.rgb, vec3<f32>(s.a)), s.a);
}

fn ui_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn ui_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let u = smoothstep(vec2<f32>(0.0), vec2<f32>(1.0), fract(p));
    let a = mix(ui_hash(i), ui_hash(i + vec2<f32>(1.0, 0.0)), u.x);
    let b = mix(ui_hash(i + vec2<f32>(0.0, 1.0)), ui_hash(i + vec2<f32>(1.0, 1.0)), u.x);
    return mix(a, b, u.y);
}

fn ui_reveal(t: f32, progress: f32, softness: f32) -> f32 {
    let s = max(softness, 1e-4);
    return clamp((clamp(progress, 0.0, 1.0) * (1.0 + s) - t) / s, 0.0, 1.0);
}
"#;
