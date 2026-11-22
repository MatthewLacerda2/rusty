mod depth;
mod emit;
mod postfx;
pub(crate) mod ui;
mod uv;
mod vertex;

use std::fmt::Write as _;

use emit::{chain, check_params, emit_helpers, emit_params, reflection_folds, Instance};

use super::blocks::{find, Stage};
use super::params::{uniform_decl, ParamLayout};
use super::recipe::{PassKind, ShaderRecipe};
use super::textures;

pub use depth::{CUT_PREPASS, CUT_SHADOW};

const SURFACE_RETURN: &str =
    "return transparent::composite(lighting_color, base_color.a, env_brdf, in.world_position, N, in.clip_position.xy);";

const FACED_IN: &str = "{\n        var in = in;\n        in.world_normal = geometric;\n        ";

pub fn assemble(recipe: &ShaderRecipe, surface_base: &str) -> Result<String, String> {
    assemble_with_params(recipe, surface_base).map(|(wgsl, _)| wgsl)
}

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
        additions.push_str(&uniform_decl());
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

/// naga_oil resolves an imported name only on lines after its `#import`, and the blocks'
/// helpers sit above the base, so the base's imports lead the variant.
fn hoist_imports(base: &str) -> (String, String) {
    let (imports, rest): (Vec<&str>, Vec<&str>) = base
        .split_inclusive('\n')
        .partition(|line| line.starts_with("#import"));
    (imports.concat(), rest.concat())
}

#[cfg(test)]
mod tests;
