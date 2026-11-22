use std::path::Path;

use super::assemble::assemble_with_params;
use super::params::sidecar_path;
use super::rebake::{engine_hash, fingerprint, recipe_path, stamp};
use super::recipe::{PassKind, ShaderRecipe};
use super::validate::validate;
use rusty_core::fs::written;

#[derive(Debug)]
pub enum BakeError {
    BaseRead(String),
    Assemble(String),
    Validate(String),
    Write(String),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::BaseRead(m) => write!(f, "shader bake: reading surface base failed: {m}"),
            BakeError::Assemble(m) => write!(f, "shader bake: assembly failed: {m}"),
            BakeError::Validate(m) => write!(f, "shader bake: validation failed: {m}"),
            BakeError::Write(m) => write!(f, "shader bake: writing module failed: {m}"),
        }
    }
}

impl std::error::Error for BakeError {}

pub fn bake_recipe(
    recipe: &ShaderRecipe,
    engine_shaders: &str,
    out_dir: &str,
) -> Result<String, BakeError> {
    let base = base_source(recipe.pass, engine_shaders).map_err(BakeError::BaseRead)?;
    let (module, params) = assemble_with_params(recipe, &base).map_err(BakeError::Assemble)?;
    validate(engine_shaders, &module).map_err(BakeError::Validate)?;
    let json =
        serde_json::to_string_pretty(&params).map_err(|e| BakeError::Write(e.to_string()))?;
    let engine = engine_hash(engine_shaders).map_err(BakeError::BaseRead)?;
    let fingerprint = fingerprint(&module, &json, engine);

    let write = |path: &Path, text: &str| {
        written::write(path, text.as_bytes())
            .map_err(|e| BakeError::Write(format!("{}: {e}", path.display())))
    };
    std::fs::create_dir_all(out_dir).map_err(|e| BakeError::Write(e.to_string()))?;
    let path = format!("{out_dir}/{}.wgsl", recipe.name);
    let wgsl = Path::new(&path);
    write(
        &recipe_path(wgsl),
        &recipe.to_json().map_err(BakeError::Write)?,
    )?;
    write(&sidecar_path(wgsl), &json)?;
    // Module last: the renderer keys on it, so its params and recipe are in place first.
    write(wgsl, &stamp(&module, fingerprint))?;
    Ok(path)
}

pub(super) fn assembled(
    recipe: &ShaderRecipe,
    engine_shaders: &str,
) -> Result<(String, String), String> {
    let base = base_source(recipe.pass, engine_shaders)?;
    let (module, params) = assemble_with_params(recipe, &base)?;
    let json = serde_json::to_string_pretty(&params).map_err(|e| e.to_string())?;
    Ok((module, json))
}

pub fn base_source(pass: PassKind, engine_shaders: &str) -> Result<String, String> {
    let file = match pass {
        PassKind::Surface => "shader.wgsl",
        PassKind::Ui => "ui.wgsl",
        PassKind::Postfx => return Ok(String::new()),
    };
    let source =
        std::fs::read_to_string(format!("{engine_shaders}/{file}")).map_err(|e| e.to_string())?;
    Ok(source.replace("\r\n", "\n"))
}

#[cfg(test)]
mod tests {
    use super::BakeError;

    #[test]
    fn a_bake_error_names_its_stage_and_carries_the_cause() {
        let cases = [
            (BakeError::BaseRead("x".into()), "reading surface base"),
            (BakeError::Assemble("x".into()), "assembly"),
            (BakeError::Validate("x".into()), "validation"),
            (BakeError::Write("x".into()), "writing module"),
        ];
        for (err, stage) in cases {
            let text = err.to_string();
            assert!(text.contains(stage) && text.ends_with(": x"), "{text}");
        }
    }
}
