//! src/api/health.rs — `Health` namespace.
//!
//! `Health.Get/Set/Heal/Damage` over `components::health::HealthComponent`.
//! `apply_damage` is shared with `Physics.Shoot`, which is hitscan-plus-damage.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Health` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_get_set(scope, &table, scene)?;
    register_heal_damage(scope, &table, scene, console)?;

    lua.globals()
        .set("Health", table)
        .map_err(|e| e.to_string())
}

/// `Get` / `Set` — read the (current, max) pair and clamp-set current health.
fn register_get_set<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Get",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let hp = scene
                .get_entity(id)
                .and_then(|e| e.health.as_ref().map(|h| (h.current_health, h.max_health)));
            Ok(hp.unwrap_or((0.0, 0.0)))
        }),
    )?;

    put(
        table,
        "Set",
        scope.create_function(|_, (id, value): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(h) = &mut e.health {
                    h.current_health = value.clamp(0.0, h.max_health);
                    h.is_dead = h.current_health <= 0.0;
                }
            }
            Ok(())
        }),
    )
}

/// `Heal` / `Damage` — add health (capped at max) or route through `apply_damage`.
fn register_heal_damage<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Heal",
        scope.create_function(|_, (id, amount): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(h) = &mut e.health {
                    h.current_health = (h.current_health + amount).min(h.max_health);
                    if h.current_health > 0.0 {
                        h.is_dead = false;
                    }
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Damage",
        scope.create_function(|_, (id, amount): (u32, f32)| {
            apply_damage(scene, console, id, amount);
            Ok(())
        }),
    )
}

/// Reduce an entity's health, flag death + freeze its death clip, and log it.
/// Shared by `Health.Damage` and `Physics.Shoot`.
pub(crate) fn apply_damage(
    scene: &RefCell<Scene>,
    console: &RefCell<ConsoleLogs>,
    id: u32,
    amount: f32,
) {
    let mut scene = scene.borrow_mut();
    let mut died: Option<String> = None;
    {
        if let Some(mut e) = scene.get_entity_mut(id) {
            let name = e.name.clone();
            if let Some(h) = &mut e.health {
                h.current_health = (h.current_health - amount).max(0.0);
                h.is_dead = h.current_health <= 0.0;
                if h.is_dead {
                    if let Some(anim) = &mut e.animator {
                        anim.current_clip = "Death".to_string();
                        anim.freeze = true;
                    }
                    died = Some(name);
                }
            }
        }
    }
    drop(scene);
    if let Some(name) = died {
        console.borrow_mut().info(format!("{} died", name));
    }
}
