//! The composite and the grading bake, assembled from [`POST_EFFECTS`] (#1467): each is
//! a fixed skeleton calling the effects' functions in their stage order, with each
//! function's slots of one params array. The source is composed through the same
//! `shadergen::compose` path as every engine shader, after the effects' modules are
//! registered ([`register_modules`]), so a broken module fails at startup, naming
//! itself.

use std::fmt::Write as _;

use super::layout::span;
use super::{composite_layout, grade_layout, Phase, Placed, POST_EFFECTS};
use crate::render::gpu::shaders::ShaderRegistry;

/// The name the composite's source composes under (it labels errors).
pub(crate) const COMPOSITE: &str = "postfx/composite (assembled)";
/// The name the bake's source composes under.
pub(crate) const BAKE: &str = "postfx/lut_bake (assembled)";

/// Register every effect's WGSL module into `registry`, once.
pub(crate) fn register_modules(registry: &mut ShaderRegistry) {
    for module in POST_EFFECTS.iter().filter_map(|e| e.module) {
        registry.add_module(module);
    }
}

/// The module `effect`'s functions live in. Panics on an effect that declares
/// functions but no module: a registry mistake, caught by the first composite.
fn module_of(effect: &super::PostEffect) -> &'static str {
    let name = effect.name;
    effect
        .module
        .unwrap_or_else(|| panic!("post effect `{name}` has functions but no module"))
}

/// The import path of the module at `file` (`postfx/effects/vignette.wgsl` →
/// `postfx::effects::vignette`): what its `#define_import_path` must say.
pub(crate) fn import_path(file: &str) -> String {
    file.trim_end_matches(".wgsl").replace('/', "::")
}

/// `#import` lines for the modules the `placed` functions live in, each once.
fn imports<F>(placed: &[Placed<F>]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    for module in placed.iter().filter_map(|p| p.effect.module) {
        if !seen.contains(&module) {
            seen.push(module);
            let _ = writeln!(out, "#import {}", import_path(module));
        }
    }
    out
}

/// `c = <module>::<function>(c, <extra>fx[i], …);`, the call of one placed function.
fn call(
    module: &str,
    function: &str,
    extra: &str,
    array: &str,
    offset: usize,
    slots: usize,
) -> String {
    let short = import_path(module);
    let short = short.rsplit("::").next().unwrap_or_default();
    let args: Vec<String> = (offset..offset + slots)
        .map(|i| format!("{array}[{i}]"))
        .collect();
    format!(
        "    c = {short}::{function}(c, {extra}{});\n",
        args.join(", ")
    )
}

/// The composite's WGSL: the fullscreen vertex stage, `fs_composite` (the scene,
/// the HDR functions, the grading LUT, the display functions) and `fs_copy`.
pub(crate) fn composite_source() -> String {
    let layout = composite_layout();
    let len = span(&layout, |c| c.slots);
    let calls = |phase: Phase| -> String {
        layout
            .iter()
            .filter(|p| p.function.phase == phase)
            .map(|p| {
                let module = module_of(p.effect);
                let f = p.function;
                call(module, f.function, "f, ", "fx", p.offset, f.slots)
            })
            .collect()
    };
    format!(
        "// The post-FX composite, assembled by `render::postfx::effects::assemble` (#1467).

#import fullscreen
#import postfx::frame::{{Frag, fragment, t_color, s_color, t_lut}}
#import postfx::grade::lut_coord
{imports}
// Every composite function's params, at the slots `effects::composite_layout` gave it.
@group(0) @binding(9) var<uniform> fx: array<vec4<f32>, {len}>;

struct VsOut {{
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {{
    var out: VsOut;
    out.uv = fullscreen::uv(vid);
    out.pos = fullscreen::triangle_down(vid);
    return out;
}}

@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {{
    let f = fragment(in.uv, in.pos.xy);
    // The scene, focused and motion-blurred upstream when the view runs them.
    var c = textureSample(t_color, s_color, in.uv).rgb;
{hdr}    // The grade + tonemap (HDR -> display): one trilinear fetch of the LUT.
    c = textureSampleLevel(t_lut, s_color, lut_coord(c), 0.0).rgb;
{display}    return vec4<f32>(c, 1.0);
}}
",
        imports = imports(&layout),
        hdr = calls(Phase::Hdr),
        display = calls(Phase::Display),
    )
}

/// The bake's WGSL: `cs_bake`, one invocation per texel of the 32³ table, storing
/// the grade functions' result for the HDR colour the texel's log code decodes to.
pub(crate) fn bake_source() -> String {
    let layout = grade_layout();
    let len = span(&layout, |g| g.slots);
    let calls: String = layout
        .iter()
        .map(|p| {
            let module = module_of(p.effect);
            let g = p.function;
            call(module, g.function, "", "g", p.offset, g.slots)
        })
        .collect();
    format!(
        "// The grading LUT's bake, assembled by `render::postfx::effects::assemble` (#1467).

#import postfx::grade::{{LUT_SIZE, decode}}
{imports}
// Every grade function's params, at the slots `effects::grade_layout` gave it.
@group(0) @binding(0) var<uniform> g: array<vec4<f32>, {len}>;
@group(0) @binding(1) var lut: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(4, 4, 4)
fn cs_bake(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (any(id >= vec3<u32>(u32(LUT_SIZE)))) {{
        return;
    }}
    var c = decode(vec3<f32>(id) / (LUT_SIZE - 1.0));
{calls}    textureStore(lut, id, vec4<f32>(c, 1.0));
}}
",
        imports = imports(&layout),
    )
}
