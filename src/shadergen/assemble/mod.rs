//! src/shadergen/assemble/mod.rs — recipe → a complete, contract-conformant
//! `.wgsl` module string (#272).
//!
//! Assembly is pure text and **deterministic**: the same recipe always produces
//! byte-identical WGSL. It is the only place blocks become source, so the output
//! is bounded to the catalog by construction.
//!
//! Three shapes, one per pass:
//! - **surface** — the canonical forward shader (`shader.wgsl`, passed in as
//!   `surface_base`) is the base; its standard `vs_main` + PBR lighting are kept
//!   verbatim and the chosen blocks are folded into the final color just before
//!   `fs_main` returns. So a surface variant binds against the forward pipeline
//!   unchanged (same `VertexInput`, bind groups, and entry points), varying only
//!   the fragment look — exactly the contract.
//! - **postfx** — a self-contained fullscreen module: the scene-color bindings,
//!   the fullscreen-triangle `vs_fullscreen`, and an `fs_main` that samples the
//!   tonemapped color then folds the chosen blocks. This matches the fullscreen-fragment
//!   shape of the postfx pass.
//! - **ui** (#427, `ui`) — the UI shader (`ui.wgsl`) is the base, kept verbatim but
//!   for `graphic()`, which folds the blocks over the graphic's shaded colour.
//!
//! A surface block that samples an extra texture slot (#400) makes the variant
//! declare that slot's binding — once, however many blocks read it; a variant with
//! no such block declares none.
//!
//! A surface block that cuts fragments (`dissolve`) also gives the variant depth-only
//! entry points, [`CUT_PREPASS`] and [`CUT_SHADOW`], so the SSAO prepass and the
//! shadow cascades drop what the colour pass drops (#648, `depth`).
//!
//! Each block instance contributes its params (as per-instance WGSL `const`s) and a
//! call spliced into the chain; each distinct block contributes its helper once —
//! all via `emit`.

mod depth;
mod emit;
pub(crate) mod ui;
mod uv;

use std::fmt::Write as _;

use emit::{chain, check_params, emit_helpers, emit_params, Instance};

use super::blocks::{find, Stage};
use super::params::{ParamLayout, UNIFORM_DECL};
use super::recipe::{PassKind, ShaderRecipe};
use super::textures;

pub use depth::{CUT_PREPASS, CUT_SHADOW};

/// The marker in the surface base shader where the assembler folds the block
/// chain into the final color. The forward shader's `fs_main` computes
/// `lighting_color`; the assembler rewrites the final write to apply the blocks
/// to it. We splice by replacing the exact final-return line, which applies the
/// scene fog (#437) last — so an authored look is fogged like every other surface.
const SURFACE_RETURN: &str = "return vec4<f32>(apply_fog(camera.fog, lighting_color, in.world_position, camera.camera_pos), base_color.a);";

/// Assemble `recipe` into a complete WGSL module. `surface_base` is the pass's base
/// shader (`bake::base_source`: the forward shader for surface, `ui.wgsl` for ui). Returns an
/// error if a selected block is unknown for the pass, or the surface base lacks
/// the expected splice point.
pub fn assemble(recipe: &ShaderRecipe, surface_base: &str) -> Result<String, String> {
    assemble_with_params(recipe, surface_base).map(|(wgsl, _)| wgsl)
}

/// [`assemble`], also returning the module's runtime-param layout (#399) — the
/// sidecar a bake writes beside it. Empty for postfx.
pub fn assemble_with_params(
    recipe: &ShaderRecipe,
    surface_base: &str,
) -> Result<(String, ParamLayout), String> {
    let resolved = resolve(recipe)?;
    let layout = ParamLayout::from_instances(recipe.pass, &resolved)?;
    let wgsl = match recipe.pass {
        PassKind::Surface => assemble_surface(recipe, &resolved, &layout, surface_base)?,
        PassKind::Postfx => assemble_postfx(recipe, &resolved, &layout),
        PassKind::Ui => ui::assemble(recipe, &resolved, &layout, surface_base)?,
    };
    Ok((wgsl, layout))
}

/// Resolve every selected block against the pass catalog and check its params,
/// erroring on the first unknown id or bad param (the gate that keeps output
/// bounded to the library and refuses typos).
fn resolve(recipe: &ShaderRecipe) -> Result<Vec<Instance<'_>>, String> {
    recipe
        .blocks
        .iter()
        .enumerate()
        .map(|(index, sel)| {
            let block = find(recipe.pass, &sel.id)
                .ok_or_else(|| format!("unknown {} block: {}", recipe.pass.tag(), sel.id))?;
            check_params(block, sel, index)?;
            Ok((block, sel))
        })
        .collect()
}

/// Assemble a surface variant by splicing the block chain into the forward base.
fn assemble_surface(
    recipe: &ShaderRecipe,
    resolved: &[Instance],
    layout: &ParamLayout,
    base: &str,
) -> Result<String, String> {
    if !base.contains(SURFACE_RETURN) {
        return Err("surface base shader is missing the expected final-return splice point".into());
    }

    let mut additions = String::new();
    let _ = writeln!(
        additions,
        "\n// ---- authored surface blocks ({}) ----",
        recipe.name
    );
    if !layout.params.is_empty() {
        additions.push_str(UNIFORM_DECL);
    }
    for slot in textures::SLOTS {
        if resolved
            .iter()
            .any(|(b, _)| b.textures.contains(&slot.name))
        {
            additions.push_str(&textures::decl(slot));
        }
    }
    emit_params(&mut additions, resolved, layout);
    emit_helpers(&mut additions, resolved);

    let folded = chain(resolved, "lighting_color", layout, Stage::Color);
    let new_return = SURFACE_RETURN.replace("lighting_color", &folded);

    let header = format!(
        "// {name}.wgsl — authored surface variant (#272), assembled from {n} block(s).\n\
         // Base: the forward/surface pass; standard vs_main + lighting kept verbatim,\n\
         // the fragment look varied by the blocks folded into the final color.\n",
        name = recipe.name,
        n = resolved.len()
    );

    let body = uv::splice(&base.replace(SURFACE_RETURN, &new_return), resolved, layout)?;
    let depth = depth::entry_points(resolved, layout);
    Ok(format!("{header}{additions}\n{body}{depth}"))
}

/// Assemble a self-contained postfx fullscreen module from the recipe.
fn assemble_postfx(recipe: &ShaderRecipe, resolved: &[Instance], layout: &ParamLayout) -> String {
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
    emit_params(&mut out, resolved, layout);
    emit_helpers(&mut out, resolved);

    let folded = chain(resolved, "color", layout, Stage::Color);
    let _ = write!(
        out,
        "\n@fragment\nfn fs_main(in: VsOut) -> @location(0) vec4<f32> {{\n    \
         let uv = in.uv;\n    \
         var color = textureSample(t_color, s_color, uv).rgb;\n    \
         color = {folded};\n    \
         return vec4<f32>(color, 1.0);\n}}\n"
    );
    out
}

/// The fixed postfx scaffolding: scene-color bindings and the fullscreen-triangle
/// vertex stage, matching `postfx.wgsl`'s self-contained fullscreen contract
/// (group(0) binding 0 = the post params, 1 = the colour so far, 2 = its sampler).
/// `game_time()` is the sim's game time (#398), the surface blocks' `camera.time`.
/// `source_tap(uv)` and `source_texel()` let a sampling block (#402) read this
/// module's input at an offset and step by whole pixels; `textureSampleLevel` keeps
/// a tap legal wherever a helper is called (no derivative uniformity rule).
const POSTFX_SCAFFOLD: &str = r#"
#import common::{PostParams}

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
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.uv = vec2<f32>(x, y);
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
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

#[cfg(test)]
mod tests;
