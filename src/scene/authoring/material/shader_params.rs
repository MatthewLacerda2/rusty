//! src/scene/authoring/material/shader_params.rs — runtime shader params (#399) on a
//! material, and per-entity overrides of them (#670, Unity's `MaterialPropertyBlock`).
//!
//! Both write paths validate the same way, once: a param the shader bakes, a typo,
//! or a wrong number of values is an error naming it ([`ParamLayout::resolve`] /
//! [`ParamLayout::coerce`]), and values are stored by canonical name so
//! `"hit_flash.amount"` and `"hit_flash.0.amount"` are one value. The material's
//! value saves with the scene; an entity's override is runtime-only
//! ([`ShaderOverrides`](crate::scene::shader_overrides::ShaderOverrides)).

use super::{ensure_material_key, MaterialLibrary};
use crate::scene::Scene;
use crate::shadergen::params::{self, ParamLayout};

/// The runtime-param layout of material `key`'s shader (#399), read from the
/// shader's `<name>.params.json` where the renderer resolves the module. Errors
/// when the material is missing, names no shader, or the shader is not baked.
pub fn shader_layout(materials: &MaterialLibrary, key: &str) -> Result<ParamLayout, String> {
    let shader = materials
        .get(key)
        .and_then(|m| m.shader.as_deref())
        .ok_or_else(|| format!("material {key:?} has no shader (Material.SetShader first)"))?;
    params::load(shader)
}

/// `name` and `value` checked against `layout`: the canonical name, and the value
/// fitted to the param's lanes (one number broadcasts to every lane).
fn checked(
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(String, Vec<f32>), String> {
    let slot = layout.resolve(name)?;
    Ok((layout.name(slot), layout.coerce(slot, value)?))
}

/// Set runtime shader param `name` (#399) on material `key`, checked against its
/// shader's `layout`. Every entity sharing the material draws with it, unless it
/// overrides the param itself.
pub fn set_shader_param(
    materials: &mut MaterialLibrary,
    key: &str,
    layout: &ParamLayout,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let (name, value) = checked(layout, name, value)?;
    if let Some(m) = materials.get_mut(key) {
        m.shader_params.insert(name, value);
    }
    Ok(())
}

/// The value runtime shader param `name` draws with on material `key`: the stored
/// one, else the shader's baked default.
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

/// Entity `id`'s material key and its shader's layout. The material is created if
/// the entity has none (like every `Material` setter), so the error is the missing
/// shader's, not the missing material's.
fn entity_layout(scene: &mut Scene, id: u32) -> Result<(String, ParamLayout), String> {
    let key = ensure_material_key(scene, id).ok_or_else(|| format!("no entity {id}"))?;
    let layout = shader_layout(&scene.materials, &key)?;
    Ok((key, layout))
}

/// Override runtime shader param `name` on entity `id` alone (#670): its material
/// stays shared and unchanged. Checked like [`set_shader_param`].
pub fn set_entity_shader_param(
    scene: &mut Scene,
    id: u32,
    name: &str,
    value: Vec<f32>,
) -> Result<(), String> {
    let (_, layout) = entity_layout(scene, id)?;
    let (name, value) = checked(&layout, name, value)?;
    scene.shader_overrides.set(id, name, value);
    Ok(())
}

/// The value runtime shader param `name` draws with on entity `id`: its override,
/// else its material's value, else the shader's baked default.
pub fn entity_shader_param(scene: &mut Scene, id: u32, name: &str) -> Result<Vec<f32>, String> {
    let (key, layout) = entity_layout(scene, id)?;
    let canonical = layout.name(layout.resolve(name)?);
    match scene
        .shader_overrides
        .of(id)
        .and_then(|o| o.get(&canonical))
    {
        Some(v) => Ok(v.clone()),
        None => shader_param(&scene.materials, &key, &layout, name),
    }
}

/// Drop entity `id`'s override of `name`, or all of its overrides when `name` is
/// `None`; it draws with its material's values again. A `name` the shader does not
/// expose at runtime is an error, as when setting it.
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
