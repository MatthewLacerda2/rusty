use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::skeleton::HitboxOptions;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GenerateHitboxes",
        lua.create_function(move |lua, (id, opts): (EntityArg, Option<Table>)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "GenerateHitboxes")?;
            let opts = options(opts)?;
            let made = scene
                .borrow_mut()
                .generate_hitboxes(id, &opts)
                .map_err(mlua::Error::RuntimeError)?;
            let out = lua.create_table()?;
            for (bone, hitbox) in made {
                out.set(bone, hitbox)?;
            }
            Ok(out)
        }),
    )
}

fn options(t: Option<Table>) -> mlua::Result<HitboxOptions> {
    let mut opts = HitboxOptions::default();
    let Some(t) = t else {
        return Ok(opts);
    };
    if let Some(bones) = t.get::<Option<Vec<String>>>("bones")? {
        opts.bones = Some(bones);
    }
    if let Some(min_size) = t.get::<Option<f32>>("min_size")? {
        opts.min_size = min_size;
    }
    if let Some(layer) = t.get::<Option<String>>("layer")? {
        opts.layer = layer;
    }
    Ok(opts)
}
