//! src/scripting/bindings/combat.rs — `Health` + `Physics` hitscan bindings.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::{global_table, put, Reg};
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// `Health.Get/Set/Heal/Damage` over the `HealthComponent`.
pub fn register_health(
    lua: &Lua,
    scene: &Rc<RefCell<Scene>>,
    console: &Rc<RefCell<ConsoleLogs>>,
) -> Reg {
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
fn apply_damage(
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

/// Extend `Physics` with `Raycast` (query) and `Shoot` (raycast + apply damage).
/// Both route through the live rapier/parry `PhysicsWorld` — the same query
/// pipeline the engine hitscan uses — so a script's cast and the engine's cast
/// return identical hits for the same ray. `Shoot` is the hitscan that used to be
/// inline in `app/play.rs`.
pub fn register_physics_hitscan(
    lua: &Lua,
    scene: &Rc<RefCell<Scene>>,
    physics: &Rc<RefCell<Option<PhysicsWorld>>>,
    console: &Rc<RefCell<ConsoleLogs>>,
) -> Reg {
    let table = global_table(lua, "Physics")?;

    let s = Rc::clone(scene);
    let p = Rc::clone(physics);
    put(
        &table,
        "Raycast",
        lua.create_function(
            move |_,
                  (ox, oy, oz, dx, dy, dz, ignore): (
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                Option<u32>,
            )| {
                // (hit, entity_id, distance) — hit=false ⇒ id/dist are 0. The
                // optional trailing `ignore` id is skipped (e.g. the shooter).
                match cast(&p, &s, Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz), ignore) {
                    Some((id, t)) => Ok((true, id, t)),
                    None => Ok((false, 0u32, 0.0f32)),
                }
            },
        ),
    )?;

    let s = Rc::clone(scene);
    let p = Rc::clone(physics);
    let c = Rc::clone(console);
    put(
        &table,
        "Shoot",
        lua.create_function(
            move |_,
                  (ox, oy, oz, dx, dy, dz, damage, ignore): (
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                Option<u32>,
            )| {
                match cast(&p, &s, Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz), ignore) {
                    Some((id, t)) => {
                        apply_damage(&s, &c, id, damage);
                        Ok((true, id, t))
                    }
                    None => Ok((false, 0u32, 0.0f32)),
                }
            },
        ),
    )?;

    Ok(())
}

/// Cast `origin`→`dir` through the live rapier world under the shared hitscan
/// filter ([`is_hittable`]), additionally skipping `ignore` (the shooter's own
/// entity, so a shot can't hit its source). Returns `None` when no physics world
/// exists yet (edit mode / no Play has built one) or on a miss.
fn cast(
    physics: &Rc<RefCell<Option<PhysicsWorld>>>,
    scene: &Rc<RefCell<Scene>>,
    origin: Vec3,
    dir: Vec3,
    ignore: Option<u32>,
) -> Option<(u32, f32)> {
    let physics = physics.borrow();
    let physics = physics.as_ref()?;
    physics.cast_ray_filtered(origin, dir, f32::MAX, |id| {
        Some(id) != ignore && is_hittable(&scene.borrow(), id)
    })
}
