//! src/api/health.rs — `Health` namespace.
//!
//! `Health.Get/Set/Heal/Damage` over `components::health::HealthComponent`.
//! `apply_damage` is shared with `Physics.Shoot`, which is hitscan-plus-damage.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Health` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>, console: &Rc<RefCell<ConsoleLogs>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "Get",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let hp = scene
                .get_entity(id)
                .and_then(|e| e.health.as_ref().map(|h| (h.current_health, h.max_health)));
            Ok(hp.unwrap_or((0.0, 0.0)))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "Set",
        lua.create_function(move |_, (id, value): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(h) = &mut e.health {
                    h.current_health = value.clamp(0.0, h.max_health);
                    h.is_dead = h.current_health <= 0.0;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "Heal",
        lua.create_function(move |_, (id, amount): (u32, f32)| {
            let mut scene = s.borrow_mut();
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

    let s = Rc::clone(scene);
    let c = Rc::clone(console);
    put(
        &table,
        "Damage",
        lua.create_function(move |_, (id, amount): (u32, f32)| {
            apply_damage(&s, &c, id, amount);
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Health", table)
        .map_err(|e| e.to_string())
}

/// Reduce an entity's health, flag death + freeze its death clip, and log it.
/// Shared by `Health.Damage` and `Physics.Shoot`.
pub(crate) fn apply_damage(
    scene: &Rc<RefCell<Scene>>,
    console: &Rc<RefCell<ConsoleLogs>>,
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
