//! src/shadergen/assemble/postfx.rs — the postfx module shape (#272): a
//! self-contained fullscreen program over the tonemapped scene colour.
//!
//! `fs_main` folds the uv-stage blocks over the fragment's `uv` first (#1679,
//! `barrel`), samples the frame there, then folds the colour blocks, which see the
//! moved `uv` too: a scanline curves with the glass, and a sampling block's centre
//! tap is the pixel the frame was sampled at. With a uv block, what lands off the
//! frame draws black, after the colour blocks. A recipe with no uv block assembles
//! exactly the text it did before uv blocks existed (`let uv = in.uv;`).

use std::fmt::Write as _;

use super::emit::{chain, emit_helpers, emit_params, Instance};
use crate::shadergen::blocks::Stage;
use crate::shadergen::params::{ParamLayout, POSTFX_UNIFORM_DECL};
use crate::shadergen::recipe::ShaderRecipe;

/// Assemble a self-contained postfx fullscreen module from the recipe.
pub fn assemble(recipe: &ShaderRecipe, resolved: &[Instance], layout: &ParamLayout) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "// {name}.wgsl — authored postfx variant (#272), assembled from {n} block(s).\n\
         // A fullscreen-triangle fragment program over the tonemapped scene color:\n\
         // samples group(0) binding(1) then folds the chosen blocks. Matches the postfx\n\
         // contract's fullscreen-fragment shape (vs_fullscreen + fs_main).",
        name = recipe.name,
        n = resolved.len()
    );
    out.push_str(POSTFX_SCAFFOLD);

    out.push_str("\n// ---- authored postfx blocks ----\n");
    if !layout.params.is_empty() {
        out.push_str(POSTFX_UNIFORM_DECL);
    }
    emit_params(&mut out, resolved, layout);
    emit_helpers(&mut out, resolved);

    let warps = resolved.iter().any(|(b, _)| b.stage == Stage::Uv);
    if warps {
        out.push_str(ON_SCREEN);
    }
    let uv = chain(resolved, "in.uv", layout, Stage::Uv);
    let folded = chain(resolved, "color", layout, Stage::Color);
    let shown = if warps {
        "color * on_screen(uv)"
    } else {
        "color"
    };
    let _ = write!(
        out,
        "\n@fragment\nfn fs_main(in: VsOut) -> @location(0) vec4<f32> {{\n    \
         let uv = {uv};\n    \
         var color = textureSample(t_color, s_color, uv).rgb;\n    \
         color = {folded};\n    \
         return vec4<f32>({shown}, 1.0);\n}}\n"
    );
    out
}

/// Emitted when a uv-stage block warps the frame (#1679): 1 where the warped `uv`
/// still lands on the frame, 0 off it, so what a warp pulls in from past the edge
/// (and a block's off-frame sentinel, `barrel`'s rounded corners) draws black after
/// every colour block, whatever they grade it to.
const ON_SCREEN: &str = "fn on_screen(uv: vec2<f32>) -> f32 {
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    return select(0.0, 1.0, inside);
}
";

/// The fixed postfx scaffolding: scene-color bindings and the fullscreen-triangle
/// vertex stage (the `fullscreen` module's triangle, imported as the engine's
/// composite does), matching the post chain's fullscreen contract
/// (group(0) binding 0 = the post params, 1 = the colour so far, 2 = its sampler).
/// `game_time()` is the sim's game time (#398), the surface blocks' `camera.time`.
/// `source_tap(uv)` and `source_texel()` let a sampling block (#402) read this
/// module's input at an offset and step by whole pixels; `textureSampleLevel` keeps
/// a tap legal wherever a helper is called (no derivative uniformity rule).
const POSTFX_SCAFFOLD: &str = r#"
#import postfx::params::PostParams
#import fullscreen

@group(0) @binding(0) var<uniform> params: PostParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    out.uv = fullscreen::uv(vid);
    out.pos = fullscreen::triangle_down(vid);
    return out;
}

fn game_time() -> f32 {
    return params.camera_pos.w;
}

fn source_texel() -> vec2<f32> {
    return 1.0 / vec2<f32>(textureDimensions(t_color));
}

fn source_tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(t_color, s_color, uv, 0.0).rgb;
}
"#;
