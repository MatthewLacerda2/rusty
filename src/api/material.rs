//! src/api/material.rs — `Material` namespace.
//!
//! `SetMetallic/Roughness/Emissive`, their map variants, and `SetTexture`. A
//! *material* is a
//! shared asset in the scene's material library (`Scene.materials`); an entity
//! references one by name via its `MaterialComponent`. These verbs resolve the
//! entity's material (creating a default one if it has none yet) and mutate the
//! asset in the library, so every entity sharing that material sees the change.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::components::{MaterialAsset, MaterialComponent, RenderMode};
use crate::scene::Scene;

/// Register the `Material` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_scalars(scope, &table, scene)?;
    register_maps(scope, &table, scene)?;
    register_transparency(scope, &table, scene)?;

    lua.globals()
        .set("Material", table)
        .map_err(|e| e.to_string())
}

/// Parse a render-mode name (case-insensitive) into a [`RenderMode`]; unknown names
/// fall back to `Opaque` so a typo degrades to the safe default rather than erroring.
fn parse_render_mode(name: &str) -> RenderMode {
    match name.to_ascii_lowercase().as_str() {
        "cutout" => RenderMode::Cutout,
        "transparent" => RenderMode::Transparent,
        _ => RenderMode::Opaque,
    }
}

/// Resolve the library key for entity `id`'s material, creating a default material
/// (under `entity_{id}_material`) and attaching the reference if it has none yet.
/// Returns `None` only when the entity does not exist.
fn ensure_material_key(scene: &mut Scene, id: u32) -> Option<String> {
    let key = {
        let mut e = scene.get_entity_mut(id)?;
        if e.material.is_none() {
            e.material = Some(MaterialComponent {
                material: format!("entity_{id}_material"),
            });
        }
        e.material.as_ref().unwrap().material.clone()
    };
    scene.materials.entry(key.clone()).or_default();
    Some(key)
}

/// Run `edit` against entity `id`'s (resolved or freshly created) library material.
fn with_material(scene: &RefCell<Scene>, id: u32, edit: impl FnOnce(&mut MaterialAsset)) {
    let mut scene = scene.borrow_mut();
    if let Some(key) = ensure_material_key(&mut scene, id) {
        if let Some(mat) = scene.materials.get_mut(&key) {
            edit(mat);
        }
    }
}

/// Scalar PBR knobs: `SetMetallic` / `SetRoughness`.
fn register_scalars<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetMetallic",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |mat| mat.metallic = val);
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughness",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |mat| mat.roughness = val);
            Ok(())
        }),
    )?;

    put(
        table,
        "SetEmissive",
        scope.create_function(|_, (id, rgb): (u32, [f32; 3])| {
            with_material(scene, id, |mat| mat.emissive = rgb);
            Ok(())
        }),
    )
}

/// Texture-map paths: `SetMetallicMap` / `SetRoughnessMap` / `SetTexture` /
/// `SetNormalMap` / `SetEmissiveMap`.
fn register_maps<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetMetallicMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |mat| mat.metallic_map = Some(path));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughnessMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |mat| mat.roughness_map = Some(path));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetTexture",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |mat| {
                mat.base_color_map = (!path.is_empty()).then_some(path);
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetNormalMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |mat| mat.normal_map = Some(path));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetEmissiveMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |mat| mat.emissive_map = Some(path));
            Ok(())
        }),
    )
}

/// Transparency controls (#242): `SetRenderMode` / `SetAlpha` / `SetAlphaCutoff`.
/// Mode is a string — "Opaque" (default), "Cutout", or "Transparent" (case-
/// insensitive); `alpha`/`cutoff` are clamped to `[0, 1]`.
fn register_transparency<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetRenderMode",
        scope.create_function(|_, (id, mode): (u32, String)| {
            with_material(scene, id, |mat| mat.render_mode = parse_render_mode(&mode));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAlpha",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |mat| mat.alpha = val.clamp(0.0, 1.0));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAlphaCutoff",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |mat| mat.alpha_cutoff = val.clamp(0.0, 1.0));
            Ok(())
        }),
    )
}
