//! crates/script/src/api/particle/render.rs — the `Particles` render setters (#440): render mode,
//! stretch, mesh, texture (#2052), flipbook, soft and lit. Each is a field write routed through the
//! shared `scene::authoring::particles` op the inspector's Render section calls.

use super::with_emitter;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_asset::MapRef;
use rusty_components::{Flipbook, ParticleRenderMode};
use rusty_scene::authoring::particles as particle_ops;
use rusty_scene::Scene;

/// Register the render setters onto the `Particles` table.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_shape(lua, table, scene)?;
    register_texture(lua, table, scene)?;
    register_look(lua, table, scene)
}

/// `SetTexture(id, path?)` / `GetTexture(id)`: the sprite image; `nil` (or `""`)
/// goes back to the default soft disc (#2052).
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

/// What each particle is drawn as: `SetRenderMode`, `GetRenderMode`, then
/// [`register_stretch_and_mesh`].
fn register_shape(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // `SetRenderMode(id, "billboard"|"stretched"|"horizontal"|"vertical"|"mesh")`.
    // Returns false (and changes nothing) for an unknown mode.
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

/// `SetStretch` and `SetMesh`: the stretched and mesh render modes' settings.
fn register_stretch_and_mesh(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // `SetStretch(id, length_scale, speed_scale?)` — quad length = size ×
    // length_scale + speed × speed_scale.
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
    // `SetMesh(id, mesh?, material?)` — a primitive name or model path, and a scene
    // material name; nil clears each.
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

/// How each particle is shaded: `SetFlipbook`, `SetSoft`, `SetLit`.
fn register_look(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // `SetFlipbook(id, columns, rows, opts?)` — opts: cycles (default 1),
    // random_start (default false). `1, 1` turns the flipbook off.
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
