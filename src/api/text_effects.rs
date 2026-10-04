//! src/api/text_effects.rs — the `Text` namespace's multi-value accessors (#419).
//!
//! The colour and the SDF effects travel as flat number lists — a few leading
//! scalars, then `r, g, b, a`: `Color` (`r, g, b, a`), `Outline` and `Glow`
//! (`width, r, g, b, a`), `Shadow` (`dx, dy, r, g, b, a`) — and auto-size as
//! `enabled, min, max`. Split from `text.rs` to keep each file one responsibility.

use std::cell::RefCell;

use mlua::{Table, Variadic};

use super::text::Effect;
use super::{put, Reg};
use crate::scene::authoring::text as ops;
use crate::scene::Scene;

/// `Get<suffix>(id)` / `Set<suffix>(id, …)` for every effect row.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    effects: &[Effect],
) -> Reg {
    for &(suffix, lead, get, set) in effects {
        put(
            table,
            &format!("Get{suffix}"),
            scope.create_function(move |_, id: u32| {
                let scene = scene.borrow();
                let v = scene.world.text(id).map_or([0.0; 6], |t| get(&t));
                Ok(Variadic::from_iter(
                    v[..lead].iter().chain(&v[2..]).copied(),
                ))
            }),
        )?;
        put(
            table,
            &format!("Set{suffix}"),
            scope.create_function(move |_, (id, args): (u32, Variadic<f32>)| {
                if args.len() < lead + 4 {
                    let msg = format!("Text.Set{suffix} expects {} numbers", lead + 4);
                    return Err(mlua::Error::RuntimeError(msg));
                }
                let mut v = [0.0; 6];
                v[..lead].copy_from_slice(&args[..lead]);
                v[2..].copy_from_slice(&args[lead..lead + 4]);
                if let Some(mut t) = scene.borrow_mut().world.text_mut(id) {
                    set(&mut t, v);
                }
                Ok(())
            }),
        )?;
    }
    Ok(())
}

/// `GetAutoSize(id) -> enabled, min, max` / `SetAutoSize(id, enabled[, min, max])`
/// (omitted bounds keep their values).
pub(super) fn register_auto_size<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetAutoSize",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let t = scene.world.text(id);
            Ok(t.map_or((false, 0.0, 0.0), |t| {
                (t.auto_size, t.auto_size_min, t.auto_size_max)
            }))
        }),
    )?;
    put(
        table,
        "SetAutoSize",
        scope.create_function(
            |_, (id, on, min, max): (u32, bool, Option<f32>, Option<f32>)| {
                if let Some(mut t) = scene.borrow_mut().world.text_mut(id) {
                    let (lo, hi) = (
                        min.unwrap_or(t.auto_size_min),
                        max.unwrap_or(t.auto_size_max),
                    );
                    ops::set_auto_size(&mut t, on, lo, hi);
                }
                Ok(())
            },
        ),
    )
}
