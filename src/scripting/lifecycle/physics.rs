//! src/scripting/lifecycle/physics.rs — dispatch the physics-step callbacks: the
//! trigger overlaps (`OnTriggerEnter` / `OnTrigger` / `OnTriggerExit`, #310) and
//! the solid contacts (`OnCollisionEnter` / `OnCollisionStay` / `OnCollisionExit`,
//! #448), both fed by `PhysicsWorld::step`'s sorted pair lists.
//!
//! Order is fixed so replays stay byte-identical: all trigger phases, then all
//! collision phases, each enter → stay → exit, each pair list ascending, and
//! every pair notifies A about B then B about A. Only awoken instances of active
//! entities are called — the one active gate every gameplay hook obeys (#323).

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::callbacks::{
    ON_COLLISION_ENTER, ON_COLLISION_EXIT, ON_COLLISION_STAY, ON_TRIGGER, ON_TRIGGER_ENTER,
    ON_TRIGGER_EXIT,
};
use super::super::manager::ScriptManager;
use crate::physics::{CollisionEvents, Contact, PhysicsEvents, TriggerEvents};

impl ScriptManager {
    /// Dispatch one physics tick's events: triggers first, then collisions.
    pub fn dispatch_physics_events(&mut self, events: PhysicsEvents) {
        if !events.triggers.is_empty() {
            self.dispatch_trigger_events(events.triggers);
        }
        if !events.collisions.is_empty() {
            self.dispatch_collision_events(events.collisions);
        }
    }

    /// Invokes the trigger callbacks on scripts of entities involved in trigger
    /// overlaps: `OnTriggerEnter` for this tick's new pairs, then `OnTrigger`
    /// (stay) for every overlapping pair, then `OnTriggerExit` for the pairs
    /// that ended (#310), each called `(id, other)`.
    pub fn dispatch_trigger_events(&mut self, events: TriggerEvents) {
        let hooks = [
            (ON_TRIGGER_ENTER, &events.entered),
            (ON_TRIGGER, &events.stayed),
            (ON_TRIGGER_EXIT, &events.exited),
        ];
        self.with_api_scope(|lua| {
            for (hook, pairs) in hooks {
                for &(a, b) in pairs {
                    self.notify_pair(lua, hook, (a, b));
                }
            }
        });
    }

    /// Invokes the collision callbacks (#448): `OnCollisionEnter(id, other,
    /// contact)` for this tick's new solid pairs, `OnCollisionStay(id, other,
    /// contact)` for every touching pair (enter tick included), then
    /// `OnCollisionExit(id, other)` for the pairs that separated. Each side gets
    /// the contact as it sees it (normal into itself, the other's velocity
    /// relative to its own).
    pub fn dispatch_collision_events(&mut self, events: CollisionEvents) {
        let touching = [
            (ON_COLLISION_ENTER, &events.entered),
            (ON_COLLISION_STAY, &events.stayed),
        ];
        self.with_api_scope(|lua| {
            for (hook, pairs) in touching {
                for pair in pairs {
                    for (id, other, contact) in pair.sides() {
                        let keys = self.awoken_keys_for(id);
                        if keys.is_empty() {
                            continue; // no listener: skip building the table
                        }
                        let Ok(table) = contact_table(lua, &contact) else {
                            continue;
                        };
                        for key in keys {
                            self.call_hook(lua, key, hook, (id, other, table.clone()));
                        }
                    }
                }
            }
            for &(a, b) in &events.exited {
                self.notify_pair(lua, ON_COLLISION_EXIT, (a, b));
            }
        });
    }

    /// Call `hook(id, other)` on both sides of a pair — A about B, then B about A
    /// — each entity's scripts in ascending script-index order.
    fn notify_pair(&self, lua: &Lua, hook: &str, (a, b): (u32, u32)) {
        for (id, other) in [(a, b), (b, a)] {
            for key in self.awoken_keys_for(id) {
                self.call_hook(lua, key, hook, (id, other));
            }
        }
    }
}

/// `{ point = {x,y,z}, normal = {x,y,z}, relativeVelocity = {x,y,z}, impulse,
/// otherBody }` — the contact a collision callback receives.
fn contact_table<'lua>(lua: &'lua Lua, c: &Contact) -> mlua::Result<Table<'lua>> {
    let vec = |v: Vec3| -> mlua::Result<Table<'lua>> {
        let t = lua.create_table()?;
        t.set("x", v.x)?;
        t.set("y", v.y)?;
        t.set("z", v.z)?;
        Ok(t)
    };
    let t = lua.create_table()?;
    t.set("point", vec(c.point)?)?;
    t.set("normal", vec(c.normal)?)?;
    t.set("relativeVelocity", vec(c.relative_velocity)?)?;
    t.set("impulse", c.impulse)?;
    t.set("otherBody", c.other_body)?;
    Ok(t)
}
