mod asset;
mod from_lua;
mod library;
pub(crate) mod shader;
mod slots;
mod surface;

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::RenderMode;
use rusty_edit::authoring::material as mat_ops;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_scalars(lua, &table, scene)?;
    register_maps(lua, &table, scene)?;
    register_transparency(lua, &table, scene)?;
    asset::register(lua, &table, scene)?;
    library::register(lua, &table, scene)?;
    slots::register(lua, &table, scene)?;
    surface::register(lua, &table, scene)?;
    shader::params::register(lua, &table, scene)?;
    shader::textures::register(lua, &table, scene)?;
    put(
        &table,
        "SetShader",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Material", "SetShader")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_shader(m, key, name)
            });
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Material", table)
        .map_err(|e| e.to_string())
}

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

fn register_scalars(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetMetallic",
        lua.create_function(move |lua, (id, val): (EntityArg, f32)| {
            let id = id.live("Material", "SetMetallic")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_metallic(m, key, val)
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRoughness",
        lua.create_function(move |lua, (id, val): (EntityArg, f32)| {
            let id = id.live("Material", "SetRoughness")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_roughness(m, key, val)
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetColor",
        lua.create_function(move |lua, (id, rgb): (EntityArg, [f32; 3])| {
            let id = id.live("Material", "SetColor")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_base_color(m, key, rgb)
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetEmissive",
        lua.create_function(move |lua, (id, rgb): (EntityArg, [f32; 3])| {
            let id = id.live("Material", "SetEmissive")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_emissive(m, key, rgb)
            });
            Ok(())
        }),
    )
}

fn register_maps(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type SetMap = fn(&mut mat_ops::MaterialLibrary, &str, String);
    let maps: [(&str, SetMap); 10] = [
        ("SetMetallicMap", |m, k, p| {
            mat_ops::set_metallic_map(m, k, Some(p))
        }),
        ("SetRoughnessMap", |m, k, p| {
            mat_ops::set_roughness_map(m, k, Some(p))
        }),
        ("SetTexture", mat_ops::set_base_color_map),
        ("SetNormalMap", |m, k, p| {
            mat_ops::set_normal_map(m, k, Some(p))
        }),
        ("SetEmissiveMap", |m, k, p| {
            mat_ops::set_emissive_map(m, k, Some(p))
        }),
        ("SetDetailMap", |m, k, p| {
            mat_ops::set_detail_map(m, k, Some(p))
        }),
        ("SetOcclusionMap", |m, k, p| {
            mat_ops::set_occlusion_map(m, k, Some(p))
        }),
        ("SetSpecularMap", |m, k, p| {
            mat_ops::set_specular_map(m, k, Some(p))
        }),
        ("SetHeightMap", |m, k, p| {
            mat_ops::set_height_map(m, k, Some(p))
        }),
        ("SetDetailMask", |m, k, p| {
            mat_ops::set_detail_mask(m, k, Some(p))
        }),
    ];
    for (verb, set) in maps {
        let f = lua.create_function(move |lua, (id, path): (EntityArg, String)| {
            let id = id.live("Material", verb)?;
            with_material(&*scene.get(lua)?, id, |m, key| set(m, key, path));
            Ok(())
        });
        put(table, verb, f)?;
    }
    let get = lua.create_function(move |lua, id: EntityArg| {
        let id = id.live("Material", "GetDetailMask")?;
        let scene = &*scene.get(lua)?;
        let scene = scene.borrow();
        let key = scene.world.material(id).map(|m| m.first().to_string());
        Ok(key.and_then(|k| mat_ops::detail_mask(&scene.materials, &k)))
    });
    put(table, "GetDetailMask", get)
}

/// `alpha`/`cutoff` are clamped to `[0, 1]`.
fn register_transparency(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetRenderMode",
        lua.create_function(move |lua, (id, mode): (EntityArg, String)| {
            let id = id.live("Material", "SetRenderMode")?;
            let mode = RenderMode::parse(&mode).unwrap_or_default();
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_render_mode(m, key, mode)
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAlpha",
        lua.create_function(move |lua, (id, val): (EntityArg, f32)| {
            let id = id.live("Material", "SetAlpha")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_alpha(m, key, val)
            });
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAlphaCutoff",
        lua.create_function(move |lua, (id, val): (EntityArg, f32)| {
            let id = id.live("Material", "SetAlphaCutoff")?;
            with_material(&*scene.get(lua)?, id, |m, key| {
                mat_ops::set_alpha_cutoff(m, key, val)
            });
            Ok(())
        }),
    )
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod detail_tests;
