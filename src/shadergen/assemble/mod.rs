//! src/shadergen/assemble/mod.rs — recipe → a complete, contract-conformant
//! `.wgsl` module string (#272).
//!
//! Assembly is pure text and **deterministic**: the same recipe always produces
//! byte-identical WGSL. It is the only place blocks become source, so the output
//! is bounded to the catalog by construction.
//!
//! Three shapes, one per pass:
//! - **surface** — the canonical forward shader (`shader.wgsl`, passed in as
//!   `surface_base`) is the base; its PBR lighting is kept verbatim and the chosen
//!   colour blocks are folded into the final color just before `surface_color`
//!   returns (scaling the SSR record of the reflection as they scale the colour,
//!   `blocks::Reflection`, #1526), which every colour entry point calls (`fs_main`,
//!   the motion-vector `fs_velocity`, #1435, and `fs_velocity_ssr`, #1526). So a
//!   surface variant binds against the
//!   forward pipeline unchanged (same `VertexInput`, bind groups, and entry points),
//!   varying only the fragment look — exactly the contract. A vertex-stage block
//!   (`vertex_waves`, #1524) replaces the base's identity `surface_vertex` hook, which
//!   every vertex stage calls (`vertex`).
//! - **postfx** — a self-contained fullscreen module: the scene-color bindings,
//!   the fullscreen-triangle `vs_fullscreen`, and an `fs_main` that samples the
//!   tonemapped color then folds the chosen blocks. This matches the fullscreen-fragment
//!   shape of the postfx pass; runtime params (#671) read the effect's group-1 uniform.
//!   A uv-stage block (`barrel`, #1679) moves where the frame is sampled (`postfx`).
//! - **ui** (#427, `ui`) — the UI shader (`ui.wgsl`) is the base, kept verbatim but
//!   for `graphic()`, which folds the blocks over the graphic's shaded colour.
//!
//! A surface block that samples an extra texture slot (#400) makes the variant
//! declare that slot's binding — once, however many blocks read it; a variant with
//! no such block declares none.
//!
//! A surface block that cuts fragments (`dissolve`), moves the UVs (`uv_scroll`) or
//! moves the vertices (`vertex_waves`) also gives the variant depth-only entry points,
//! [`CUT_PREPASS`] and [`CUT_SHADOW`], so the SSAO prepass and the shadow cascades
//! draw what the colour pass draws (#648, #1499, #1524, `depth`).
//!
//! Each block instance contributes its params (as per-instance WGSL `const`s) and a
//! call spliced into the chain; each distinct block contributes its helper once —
//! all via `emit`.

mod depth;
mod emit;
mod postfx;
pub(crate) mod ui;
mod uv;
mod vertex;

use std::fmt::Write as _;

use emit::{chain, check_params, emit_helpers, emit_params, reflection_folds, Instance};

use super::blocks::{find, Stage};
use super::params::{ParamLayout, UNIFORM_DECL};
use super::recipe::{PassKind, ShaderRecipe};
use super::textures;

pub use depth::{CUT_PREPASS, CUT_SHADOW};

/// The marker in the surface base shader where the assembler folds the block
/// chain into the final color. The forward shader's `surface_color` computes
/// `lighting_color`; the assembler rewrites the final write to apply the blocks
/// to it. We splice by replacing the exact final-return line, which applies the
/// scene fog (#437) and the transparent composite (#1523) last — so an authored
/// look is fogged and blended like every other surface.
const SURFACE_RETURN: &str =
    "return transparent::composite(lighting_color, base_color.a, env_brdf, in.world_position, N, in.clip_position.xy);";

/// Opens the block the colour blocks run in: `in` shadowed by a copy carrying the
/// shading's facing-corrected geometric normal (`surface_color`'s `geometric`).
const FACED_IN: &str = "{\n        var in = in;\n        in.world_normal = geometric;\n        ";

/// Assemble `recipe` into a complete WGSL module. `surface_base` is the pass's base
/// shader (`bake::base_source`: the forward shader for surface, `ui.wgsl` for ui). Returns an
/// error if a selected block is unknown for the pass, or the surface base lacks
/// the expected splice point.
pub fn assemble(recipe: &ShaderRecipe, surface_base: &str) -> Result<String, String> {
    assemble_with_params(recipe, surface_base).map(|(wgsl, _)| wgsl)
}

/// [`assemble`], also returning the module's runtime-param layout (#399) — the
/// sidecar a bake writes beside it.
pub fn assemble_with_params(
    recipe: &ShaderRecipe,
    surface_base: &str,
) -> Result<(String, ParamLayout), String> {
    let resolved = resolve(recipe)?;
    let layout = ParamLayout::from_instances(recipe.pass, &resolved)?;
    let wgsl = match recipe.pass {
        PassKind::Surface => assemble_surface(recipe, &resolved, &layout, surface_base)?,
        PassKind::Postfx => postfx::assemble(recipe, &resolved, &layout),
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
                .ok_or_else(|| format!("unknown {} block: {}", recipe.pass.name(), sel.id))?;
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

    // The blocks restyle the colour after step 5 recorded its reflection for SSR, so
    // the record is scaled as they scale the colour, right before it (#1526).
    // They read the vertex output through a copy whose normal is the one the
    // surface is shaded with, turned toward the viewer on a two-sided face seen from
    // behind (#1615): a rim seen from behind is the rim of the face the viewer sees.
    let folded = chain(resolved, "lighting_color", layout, Stage::Color);
    let mut new_return = String::from(FACED_IN);
    for fold in reflection_folds(resolved, layout) {
        let _ = write!(new_return, "{fold}\n        ");
    }
    new_return.push_str(&SURFACE_RETURN.replace("lighting_color", &folded));
    new_return.push_str("\n    }");

    let header = format!(
        "// {name}.wgsl — authored surface variant (#272), assembled from {n} block(s).\n\
         // Base: the forward/surface pass; lighting kept verbatim, the look varied by\n\
         // the blocks folded into the final color, the UVs or the vertex hook.\n",
        name = recipe.name,
        n = resolved.len()
    );

    let body = uv::splice(&base.replace(SURFACE_RETURN, &new_return), resolved, layout)?;
    let body = vertex::splice(&body, resolved, layout)?;
    let (imports, body) = hoist_imports(&body);
    let depth = depth::entry_points(resolved, layout);
    Ok(format!("{header}{imports}{additions}\n{body}{depth}"))
}

/// Split the base's `#import` lines from the rest. They lead the variant: naga_oil
/// resolves an imported name only on lines after its `#import`, and the blocks'
/// helpers, emitted above the base, read the forward bindings the base imports
/// (`camera`, `entity`, …, #1015).
fn hoist_imports(base: &str) -> (String, String) {
    let (imports, rest): (Vec<&str>, Vec<&str>) = base
        .split_inclusive('\n')
        .partition(|line| line.starts_with("#import"));
    (imports.concat(), rest.concat())
}

#[cfg(test)]
mod tests;
