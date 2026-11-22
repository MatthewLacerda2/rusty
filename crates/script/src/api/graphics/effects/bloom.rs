use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_edit::authoring::rendering::post_processing::bloom;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let active = |vc: &rusty_components::PostProcessingComponent| vc.bloom.active;
    super::knobs::register_flag(
        lua,
        (table, scene),
        "BloomActive",
        active,
        bloom::set_active,
    )?;
    put(
        table,
        "SetBloomDirtTexture",
        lua.create_function(move |lua, path: Option<String>| {
            with_vc_mut(&*scene.get(lua)?, |e| bloom::set_dirt_texture(e, path));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBloomDirtTexture",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.bloom.dirt_texture.clone()).flatten())
        }),
    )?;
    super::knobs::register_quality(
        lua,
        (table, scene),
        "BloomQuality",
        |vc| vc.bloom.quality,
        bloom::set_quality,
    )?;
    super::knobs::register_count(
        lua,
        (table, scene),
        "BloomResolutionDivisor",
        |vc| vc.bloom.resolution_divisor,
        bloom::set_resolution_divisor,
    )?;
    super::knobs::register(lua, table, scene, bloom::EFFECT.knobs)
}
