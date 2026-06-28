//! src/api/material/mod.rs — `Material` namespace.
//!
//! Two authoring surfaces over one shared material model:
//!
//! 1. **Per-entity setters** — `SetMetallic/Roughness/Emissive`, their map variants,
//!    `SetTexture`, and the transparency knobs. A *material* is a shared asset in the
//!    scene's material library (`Scene.materials`); an entity references one by name via
//!    its `MaterialComponent`. These verbs resolve the entity's material (creating a
//!    default one if it has none yet) and mutate the asset, so every entity sharing it
//!    sees the change.
//! 2. **Standalone named-asset authoring** ([`asset`], #271) — `DefineAsset` /
//!    `DefineAssetJson` / `GetAsset` / `HasAsset`: author a reusable `MaterialAsset`
//!    into the library under a chosen *name*, decoupled from any entity. An entity then
//!    uses it by pointing its `MaterialComponent.material` at that name. Round-trips
//!    through `SceneData` like any library asset.
//!
//! Both are THIN adapters (#287): each setter resolves a library key (the entity's via
//! `authoring::material::ensure_material_key`, or the chosen name via
//! `authoring::material::ensure_named`), parses the render-mode string, then calls the
//! matching `authoring::material::*` op. The field write + validation (e.g. the `[0, 1]`
//! alpha clamp) lives ONCE in that shared module, which the editor's Material card calls
//! too — egui panel and Lua binding are siblings over the same Rust op.

mod asset;
mod from_lua;

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::components::RenderMode;
use crate::scene::authoring::material as mat_ops;
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
    asset::register(scope, &table, scene)?;

    lua.globals()
        .set("Material", table)
        .map_err(|e| e.to_string())
}

/// Parse a render-mode name (case-insensitive) into a [`RenderMode`]; unknown names
/// fall back to `Opaque` so a typo degrades to the safe default rather than erroring.
/// Shared with [`asset`] so the standalone recipe parses modes identically.
pub(super) fn parse_render_mode(name: &str) -> RenderMode {
    match name.to_ascii_lowercase().as_str() {
        "cutout" => RenderMode::Cutout,
        "transparent" => RenderMode::Transparent,
        _ => RenderMode::Opaque,
    }
}

/// Resolve entity `id`'s (resolved or freshly created) library material key and run
/// `apply` against the shared library + key. The resolve-or-create is the shared
/// `authoring::material::ensure_material_key`; `apply` is one of the shared
/// `authoring::material::*` ops, so the field write + validation is never duplicated.
fn with_material(
    scene: &RefCell<Scene>,
    id: u32,
    apply: impl FnOnce(&mut mat_ops::MaterialLibrary, &str),
) {
    let mut scene = scene.borrow_mut();
    if let Some(key) = mat_ops::ensure_material_key(&mut scene, id) {
        apply(&mut scene.materials, &key);
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
            with_material(scene, id, |m, key| mat_ops::set_metallic(m, key, val));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughness",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |m, key| mat_ops::set_roughness(m, key, val));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetEmissive",
        scope.create_function(|_, (id, rgb): (u32, [f32; 3])| {
            with_material(scene, id, |m, key| mat_ops::set_emissive(m, key, rgb));
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
            with_material(scene, id, |m, key| {
                mat_ops::set_metallic_map(m, key, Some(path))
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughnessMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |m, key| {
                mat_ops::set_roughness_map(m, key, Some(path))
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetTexture",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |m, key| {
                mat_ops::set_base_color_map(m, key, path)
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetNormalMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |m, key| {
                mat_ops::set_normal_map(m, key, Some(path))
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetEmissiveMap",
        scope.create_function(|_, (id, path): (u32, String)| {
            with_material(scene, id, |m, key| {
                mat_ops::set_emissive_map(m, key, Some(path))
            });
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
            let mode = parse_render_mode(&mode);
            with_material(scene, id, |m, key| mat_ops::set_render_mode(m, key, mode));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAlpha",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |m, key| mat_ops::set_alpha(m, key, val));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAlphaCutoff",
        scope.create_function(|_, (id, val): (u32, f32)| {
            with_material(scene, id, |m, key| mat_ops::set_alpha_cutoff(m, key, val));
            Ok(())
        }),
    )
}
