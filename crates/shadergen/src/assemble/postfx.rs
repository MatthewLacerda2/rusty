use std::fmt::Write as _;

use super::emit::{chain, emit_helpers, emit_params, Instance};
use crate::blocks::Stage;
use crate::params::{ParamLayout, POSTFX_UNIFORM_DECL};
use crate::recipe::ShaderRecipe;

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

const ON_SCREEN: &str = "fn on_screen(uv: vec2<f32>) -> f32 {
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    return select(0.0, 1.0, inside);
}
";

/// Group 0 is render's post-chain IO layout (`create_io_layout`): params, the colour so
/// far, its sampler. `source_tap` takes no derivatives, so a tap is legal in any branch.
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
