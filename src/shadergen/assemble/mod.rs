//! src/shadergen/assemble/mod.rs — recipe → a complete, contract-conformant
//! `.wgsl` module string (#272).
//!
//! Assembly is pure text and **deterministic**: the same recipe always produces
//! byte-identical WGSL. It is the only place blocks become source, so the output
//! is bounded to the catalog by construction.
//!
//! Two shapes, one per pass:
//! - **surface** — the canonical forward shader (`shader.wgsl`, passed in as
//!   `surface_base`) is the base; its standard `vs_main` + PBR lighting are kept
//!   verbatim and the chosen blocks are folded into the final color just before
//!   `fs_main` returns. So a surface variant binds against the forward pipeline
//!   unchanged (same `VertexInput`, bind groups, and entry points), varying only
//!   the fragment look — exactly the contract.
//! - **postfx** — a self-contained fullscreen module: the scene-color bindings,
//!   the fullscreen-triangle `vs_fullscreen`, and an `fs_main` that samples the
//!   HDR color then folds the chosen blocks. This matches the fullscreen-fragment
//!   shape of the postfx pass.
//!
//! Each block contributes a param block (its declared params as WGSL `const`s),
//! a helper function, and a call spliced into the chain — all via [`emit`].

mod emit;

use std::fmt::Write as _;

use emit::{chain, emit_params};

use super::blocks::{find, Block};
use super::recipe::{BlockSel, PassKind, ShaderRecipe};

/// The marker in the surface base shader where the assembler folds the block
/// chain into the final color. The forward shader's `fs_main` computes
/// `lighting_color`; the assembler rewrites the final write to apply the blocks
/// to it. We splice by replacing the exact final-return line.
const SURFACE_RETURN: &str = "return vec4<f32>(lighting_color, base_color.a);";

/// Assemble `recipe` into a complete WGSL module. `surface_base` is the canonical
/// forward shader source (only consulted for [`PassKind::Surface`]). Returns an
/// error if a selected block is unknown for the pass, or the surface base lacks
/// the expected splice point.
pub fn assemble(recipe: &ShaderRecipe, surface_base: &str) -> Result<String, String> {
    match recipe.pass {
        PassKind::Surface => assemble_surface(recipe, surface_base),
        PassKind::Postfx => assemble_postfx(recipe),
    }
}

/// Resolve every selected block against the pass catalog, erroring on the first
/// unknown id (the gate that keeps output bounded to the library).
fn resolve(recipe: &ShaderRecipe) -> Result<Vec<(&'static Block, &BlockSel)>, String> {
    recipe
        .blocks
        .iter()
        .map(|sel| {
            find(recipe.pass, &sel.id)
                .map(|b| (b, sel))
                .ok_or_else(|| format!("unknown {} block: {}", recipe.pass.tag(), sel.id))
        })
        .collect()
}

/// Assemble a surface variant by splicing the block chain into the forward base.
fn assemble_surface(recipe: &ShaderRecipe, base: &str) -> Result<String, String> {
    if !base.contains(SURFACE_RETURN) {
        return Err("surface base shader is missing the expected final-return splice point".into());
    }
    let resolved = resolve(recipe)?;

    let mut additions = String::new();
    let _ = writeln!(
        additions,
        "\n// ---- authored surface blocks ({}) ----",
        recipe.name
    );
    for (block, sel) in &resolved {
        emit_params(&mut additions, block, sel);
    }
    for (block, _) in &resolved {
        let _ = writeln!(additions, "{}", block.helper);
    }

    let folded = chain(&resolved, "lighting_color");
    let new_return = format!("return vec4<f32>({folded}, base_color.a);");

    let header = format!(
        "// {name}.wgsl — authored surface variant (#272), assembled from {n} block(s).\n\
         // Base: the forward/surface pass; standard vs_main + lighting kept verbatim,\n\
         // the fragment look varied by the blocks folded into the final color.\n",
        name = recipe.name,
        n = resolved.len()
    );

    let body = base.replace(SURFACE_RETURN, &new_return);
    Ok(format!("{header}{additions}\n{body}"))
}

/// Assemble a self-contained postfx fullscreen module from the recipe.
fn assemble_postfx(recipe: &ShaderRecipe) -> Result<String, String> {
    let resolved = resolve(recipe)?;
    let mut out = String::new();

    let _ = writeln!(
        out,
        "// {name}.wgsl — authored postfx variant (#272), assembled from {n} block(s).\n\
         // A fullscreen-triangle fragment program over the HDR scene color: samples\n\
         // group(0) binding(1) then folds the chosen blocks. Matches the postfx\n\
         // contract's fullscreen-fragment shape (vs_fullscreen + fs_main).",
        name = recipe.name,
        n = resolved.len()
    );
    out.push_str(POSTFX_SCAFFOLD);

    out.push_str("\n// ---- authored postfx blocks ----\n");
    for (block, sel) in &resolved {
        emit_params(&mut out, block, sel);
    }
    for (block, _) in &resolved {
        let _ = writeln!(out, "{}", block.helper);
    }

    let folded = chain(&resolved, "color");
    let _ = write!(
        out,
        "\n@fragment\nfn fs_main(in: VsOut) -> @location(0) vec4<f32> {{\n    \
         let uv = in.uv;\n    \
         var color = textureSample(t_color, s_color, uv).rgb;\n    \
         color = {folded};\n    \
         return vec4<f32>(color, 1.0);\n}}\n"
    );
    Ok(out)
}

/// The fixed postfx scaffolding: scene-color bindings and the fullscreen-triangle
/// vertex stage, matching `postfx.wgsl`'s self-contained fullscreen contract
/// (group(0) binding 1 = HDR color, binding 2 = its sampler).
const POSTFX_SCAFFOLD: &str = r#"
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
"#;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::recipe::ParamValue;
    use super::*;

    fn surface_base() -> &'static str {
        // A minimal stand-in containing the splice point, so assembler unit tests
        // don't read the real asset (the bake/validate tests do that end-to-end).
        "fn fs_main() -> vec4<f32> {\n    var lighting_color = vec3<f32>(1.0);\n    var base_color = vec4<f32>(1.0);\n    return vec4<f32>(lighting_color, base_color.a);\n}"
    }

    #[test]
    fn surface_folds_blocks_into_the_final_return() {
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: "t".into(),
            blocks: vec![BlockSel {
                id: "toon_ramp".into(),
                params: BTreeMap::new(),
            }],
        };
        let wgsl = assemble(&recipe, surface_base()).unwrap();
        assert!(wgsl.contains("const toon_ramp_steps: f32 = 4.0;"));
        assert!(wgsl.contains("fn srf_toon_ramp"));
        assert!(wgsl.contains("srf_toon_ramp(lighting_color, in)"));
        // The original flat return is gone (rewritten to apply the chain).
        assert!(!wgsl.contains("return vec4<f32>(lighting_color, base_color.a);"));
    }

    #[test]
    fn postfx_emits_fullscreen_entry_points_and_chain() {
        let mut params = BTreeMap::new();
        params.insert(
            "color".to_string(),
            ParamValue::Vector(vec![1.0, 0.5, 0.25]),
        );
        let recipe = ShaderRecipe {
            pass: PassKind::Postfx,
            name: "t".into(),
            blocks: vec![BlockSel {
                id: "tint".into(),
                params,
            }],
        };
        let wgsl = assemble(&recipe, "").unwrap();
        assert!(wgsl.contains("fn vs_fullscreen"));
        assert!(wgsl.contains("fn fs_main"));
        assert!(wgsl.contains("const tint_color: vec3<f32> = vec3<f32>(1.0, 0.5, 0.25);"));
        assert!(wgsl.contains("color = pfx_tint(color, uv);"));
    }

    #[test]
    fn unknown_block_is_rejected() {
        let recipe = ShaderRecipe {
            pass: PassKind::Postfx,
            name: "t".into(),
            blocks: vec![BlockSel {
                id: "nope".into(),
                params: BTreeMap::new(),
            }],
        };
        assert!(assemble(&recipe, "").is_err());
    }

    #[test]
    fn assembly_is_deterministic() {
        let recipe = ShaderRecipe {
            pass: PassKind::Postfx,
            name: "t".into(),
            blocks: vec![
                BlockSel {
                    id: "vignette".into(),
                    params: BTreeMap::new(),
                },
                BlockSel {
                    id: "scanline".into(),
                    params: BTreeMap::new(),
                },
            ],
        };
        let a = assemble(&recipe, "").unwrap();
        let b = assemble(&recipe, "").unwrap();
        assert_eq!(a, b);
    }
}
