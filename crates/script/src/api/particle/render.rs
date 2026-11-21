use super::with_emitter;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_asset::MapRef;
use rusty_components::{Flipbook, ParticleRenderMode};
use rusty_edit::authoring::particles as particle_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_shape(lua, table, scene)?;
    register_texture(lua, table, scene)?;
    register_look(lua, table, scene)
}

fn register_texture(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetTexture",
        lua.create_function(move |lua, (id, path): (EntityArg, Option<String>)| {
            let id = id.live("Particles", "SetTexture")?;
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_texture(p, path.unwrap_or_default())
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTexture",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let particles = scene.world.particles(id.read());
            Ok(particles.and_then(|p| p.texture.as_ref().map(MapRef::to_string)))
        }),
    )
}

fn register_shape(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetRenderMode",
        lua.create_function(move |lua, (id, mode): (EntityArg, String)| {
            let id = id.live("Particles", "SetRenderMode")?;
            let Some(mode) = ParticleRenderMode::parse(&mode) else {
                return Ok(false);
            };
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_render_mode(p, mode)
            });
            Ok(true)
        }),
    )?;
    put(
        table,
        "GetRenderMode",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let mode = scene
                .world
                .particles(id.read())
                .map(|p| p.render.mode.name());
            Ok(mode)
        }),
    )?;
    register_stretch_and_mesh(lua, table, scene)
}

fn register_stretch_and_mesh(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetStretch",
        lua.create_function(
            move |lua, (id, length, speed): (EntityArg, f32, Option<f32>)| {
                let id = id.live("Particles", "SetStretch")?;
                with_emitter(&*scene.get(lua)?, id, |p| {
                    particle_ops::set_stretch(p, length, speed.unwrap_or(0.0))
                });
                Ok(())
            },
        ),
    )?;
    put(
        table,
        "SetMesh",
        lua.create_function(
            move |lua, (id, mesh, material): (EntityArg, Option<String>, Option<String>)| {
                let id = id.live("Particles", "SetMesh")?;
                with_emitter(&*scene.get(lua)?, id, |p| {
                    particle_ops::set_render_mesh(p, mesh.unwrap_or_default());
                    particle_ops::set_render_material(p, material.unwrap_or_default());
                });
                Ok(())
            },
        ),
    )
}

fn register_look(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetFlipbook",
        lua.create_function(
            move |lua, (id, columns, rows, opts): (EntityArg, u32, u32, Option<mlua::Table>)| {
                let id = id.live("Particles", "SetFlipbook")?;
                let cycles = opts.as_ref().and_then(|o| o.get("cycles").ok());
                let random_start = opts.as_ref().and_then(|o| o.get("random_start").ok());
                let flipbook = Flipbook {
                    columns,
                    rows,
                    cycles: cycles.unwrap_or(1.0),
                    random_start: random_start.unwrap_or(false),
                };
                with_emitter(&*scene.get(lua)?, id, |p| {
                    particle_ops::set_flipbook(p, flipbook)
                });
                Ok(())
            },
        ),
    )?;
    put(
        table,
        "SetSoft",
        lua.create_function(move |lua, (id, distance): (EntityArg, f32)| {
            let id = id.live("Particles", "SetSoft")?;
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_soft_distance(p, distance)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "SetLit",
        lua.create_function(move |lua, (id, lit): (EntityArg, bool)| {
            let id = id.live("Particles", "SetLit")?;
            with_emitter(&*scene.get(lua)?, id, |p| particle_ops::set_lit(p, lit));
            Ok(())
        }),
    )
}
