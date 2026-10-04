//! src/api/particle/render.rs — the `Particles` render setters (#440): render mode,
//! stretch, mesh, flipbook, soft and lit. Each is a field write routed through the
//! shared `scene::authoring::particles` op the inspector's Render section calls.

use std::cell::RefCell;

use super::with_emitter;
use crate::api::{put, Reg};
use crate::components::{Flipbook, ParticleRenderMode};
use crate::scene::authoring::particles as particle_ops;
use crate::scene::Scene;

/// Register the render setters onto the `Particles` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_shape(scope, table, scene)?;
    register_look(scope, table, scene)
}

/// What each particle is drawn as: `SetRenderMode`, `GetRenderMode`, `SetStretch`,
/// `SetMesh`.
fn register_shape<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    // `SetRenderMode(id, "billboard"|"stretched"|"horizontal"|"vertical"|"mesh")`.
    // Returns false (and changes nothing) for an unknown mode.
    put(
        table,
        "SetRenderMode",
        scope.create_function(|_, (id, mode): (u32, String)| {
            let Some(mode) = ParticleRenderMode::parse(&mode) else {
                return Ok(false);
            };
            with_emitter(scene, id, |p| particle_ops::set_render_mode(p, mode));
            Ok(true)
        }),
    )?;
    put(
        table,
        "GetRenderMode",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let mode = scene.world.particles(id).map(|p| p.render.mode.name());
            Ok(mode)
        }),
    )?;
    // `SetStretch(id, length_scale, speed_scale?)` — quad length = size ×
    // length_scale + speed × speed_scale.
    put(
        table,
        "SetStretch",
        scope.create_function(|_, (id, length, speed): (u32, f32, Option<f32>)| {
            with_emitter(scene, id, |p| {
                particle_ops::set_stretch(p, length, speed.unwrap_or(0.0))
            });
            Ok(())
        }),
    )?;
    // `SetMesh(id, mesh?, material?)` — a primitive name or model path, and a scene
    // material name; nil clears each.
    put(
        table,
        "SetMesh",
        scope.create_function(
            |_, (id, mesh, material): (u32, Option<String>, Option<String>)| {
                with_emitter(scene, id, |p| {
                    particle_ops::set_render_mesh(p, mesh.unwrap_or_default());
                    particle_ops::set_render_material(p, material.unwrap_or_default());
                });
                Ok(())
            },
        ),
    )
}

/// How each particle is shaded: `SetFlipbook`, `SetSoft`, `SetLit`.
fn register_look<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    // `SetFlipbook(id, columns, rows, opts?)` — opts: cycles (default 1),
    // random_start (default false). `1, 1` turns the flipbook off.
    put(
        table,
        "SetFlipbook",
        scope.create_function(
            |_, (id, columns, rows, opts): (u32, u32, u32, Option<mlua::Table>)| {
                let cycles = opts.as_ref().and_then(|o| o.get("cycles").ok());
                let random_start = opts.as_ref().and_then(|o| o.get("random_start").ok());
                let flipbook = Flipbook {
                    columns,
                    rows,
                    cycles: cycles.unwrap_or(1.0),
                    random_start: random_start.unwrap_or(false),
                };
                with_emitter(scene, id, |p| particle_ops::set_flipbook(p, flipbook));
                Ok(())
            },
        ),
    )?;
    put(
        table,
        "SetSoft",
        scope.create_function(|_, (id, distance): (u32, f32)| {
            with_emitter(scene, id, |p| particle_ops::set_soft_distance(p, distance));
            Ok(())
        }),
    )?;
    put(
        table,
        "SetLit",
        scope.create_function(|_, (id, lit): (u32, bool)| {
            with_emitter(scene, id, |p| particle_ops::set_lit(p, lit));
            Ok(())
        }),
    )
}
