//! Values are stored by canonical name, so `"hit_flash.amount"` and `"hit_flash.0.amount"` are one
//! value. The material's value saves with the scene; an entity's override is runtime-only.

use super::{ensure_material_key, MaterialLibrary};
use rusty_scene::rules::sanitize::finite_values;
use rusty_scene::Scene;
use rusty_shadergen::params::{self, ParamLayout};

pub fn shader_layout(materials: &MaterialLibrary, key: &str) -> Result<ParamLayout, String> {
    let shader = materials
        .get(key)
        .and_then(|m| m.shader.as_deref())
        .ok_or_else(|| format!("material {key:?} has no shader (Material.SetShader first)"))?;
    params::load(shader)
}

fn checked(
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(String, Vec<f32>), String> {
    let slot = layout.resolve(name)?;
    Ok((layout.name(slot), layout.coerce(slot, value)?))
}

pub fn set_shader_param(
    materials: &mut MaterialLibrary,
    key: &str,
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let (name, value) = checked(layout, name, finite_values(value)?)?;
    if let Some(m) = materials.get_mut(key) {
        m.shader_params.insert(name, value);
    }
    Ok(())
}

pub fn shader_param(
    materials: &MaterialLibrary,
    key: &str,
    layout: &ParamLayout,
    name: &str,
) -> Result<Vec<f32>, String> {
    let slot = layout.resolve(name)?;
    let stored = materials
        .get(key)
        .and_then(|m| m.shader_params.get(&layout.name(slot)));
    Ok(stored.cloned().unwrap_or_else(|| slot.default.clone()))
}

/// The material is created if the entity has none (like every `Material` setter), so the error is
/// the missing shader's.
fn entity_layout(scene: &mut Scene, id: u32) -> Result<(String, ParamLayout), String> {
    let key = ensure_material_key(scene, id).ok_or_else(|| format!("no entity {id}"))?;
    let layout = shader_layout(&scene.materials, &key)?;
    Ok((key, layout))
}

pub fn set_entity_shader_param(
    scene: &mut Scene,
    id: u32,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let (_, layout) = entity_layout(scene, id)?;
    set_entity_shader_param_with(scene, id, &layout, name, value)
}

pub fn set_entity_shader_param_with(
    scene: &mut Scene,
    id: u32,
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let (name, value) = checked(layout, name, finite_values(value)?)?;
    scene.shader_overrides.set(id, name, value);
    Ok(())
}

pub fn entity_shader_param(scene: &mut Scene, id: u32, name: &str) -> Result<Vec<f32>, String> {
    let (_, layout) = entity_layout(scene, id)?;
    entity_shader_param_with(scene, id, &layout, name)
}

pub fn entity_shader_param_with(
    scene: &Scene,
    id: u32,
    layout: &ParamLayout,
    name: &str,
) -> Result<Vec<f32>, String> {
    let canonical = layout.name(layout.resolve(name)?);
    if let Some(v) = scene
        .shader_overrides
        .of(id)
        .and_then(|o| o.get(&canonical))
    {
        return Ok(v.clone());
    }
    let key = scene.world.material(id).map(|m| m.first().to_string());
    shader_param(&scene.materials, &key.unwrap_or_default(), layout, name)
}

pub fn clear_entity_shader_param(
    scene: &mut Scene,
    id: u32,
    name: Option<&str>,
) -> Result<(), String> {
    let Some(name) = name else {
        if !scene.world.contains(id) {
            return Err(format!("no entity {id}"));
        }
        scene.shader_overrides.clear(id, None);
        return Ok(());
    };
    let (_, layout) = entity_layout(scene, id)?;
    let canonical = layout.name(layout.resolve(name)?);
    scene.shader_overrides.clear(id, Some(&canonical));
    Ok(())
}

/// Whether the module exists is the renderer's call (it falls back, logging once), so a shader
/// baked later still binds.
pub fn set_shader(materials: &mut MaterialLibrary, key: &str, name: String) {
    if let Some(m) = materials.get_mut(key) {
        m.shader = (!name.is_empty()).then_some(name);
    }
}

/// The path is not checked: a missing file samples white, and a texture made later binds when it
/// appears.
pub fn set_shader_texture(
    materials: &mut MaterialLibrary,
    key: &str,
    slot: &str,
    path: Option<String>,
) -> Result<(), String> {
    rusty_shadergen::textures::slot(slot)?;
    if let Some(m) = materials.get_mut(key) {
        match path.filter(|p| !p.is_empty()) {
            Some(p) => m.shader_textures.insert(slot.to_owned(), p.into()),
            None => m.shader_textures.remove(slot),
        };
    }
    Ok(())
}
