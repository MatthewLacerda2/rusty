use glam::Vec3;
use mlua::{Lua, Table};

use super::super::callbacks::ON_JOINT_BREAK;
use super::super::callbacks::{
    ON_COLLISION_ENTER, ON_COLLISION_EXIT, ON_COLLISION_STAY, ON_TRIGGER, ON_TRIGGER_ENTER,
    ON_TRIGGER_EXIT,
};
use super::super::manager::ScriptManager;
use rusty_physics::{CollisionEvents, Contact, JointBreak, PhysicsEvents, TriggerEvents};

impl ScriptManager {
    pub fn dispatch_physics_events(&mut self, events: PhysicsEvents) {
        if !events.triggers.is_empty() {
            self.dispatch_trigger_events(events.triggers);
        }
        if !events.collisions.is_empty() {
            self.dispatch_collision_events(events.collisions);
        }
        if !events.joint_breaks.is_empty() {
            self.dispatch_joint_breaks(&events.joint_breaks);
        }
    }

    fn dispatch_joint_breaks(&mut self, breaks: &[JointBreak]) {
        self.with_lua(|lua| {
            for b in breaks {
                for key in self.awoken_keys_for(b.id) {
                    self.call_hook(lua, key, ON_JOINT_BREAK, (b.id, b.force, b.torque));
                }
            }
        });
    }

    pub fn dispatch_trigger_events(&mut self, events: TriggerEvents) {
        let hooks = [
            (ON_TRIGGER_ENTER, &events.entered),
            (ON_TRIGGER, &events.stayed),
            (ON_TRIGGER_EXIT, &events.exited),
        ];
        self.with_lua(|lua| {
            for (hook, pairs) in hooks {
                for &(a, b) in pairs {
                    self.notify_pair(lua, hook, (a, b));
                }
            }
        });
    }

    pub fn dispatch_collision_events(&mut self, events: CollisionEvents) {
        let touching = [
            (ON_COLLISION_ENTER, &events.entered),
            (ON_COLLISION_STAY, &events.stayed),
        ];
        self.with_lua(|lua| {
            let scene = crate::api::ApiCell::<rusty_scene::Scene>::new()
                .get(lua)
                .ok();
            for (hook, pairs) in touching {
                for pair in pairs {
                    for (id, other, contact) in pair.sides() {
                        let keys = self.awoken_keys_for(id);
                        if keys.is_empty() {
                            continue; // no listener: skip building the table
                        }
                        let material = scene.as_ref().and_then(|s| material_of(&s.borrow(), other));
                        let Ok(table) = contact_table(lua, &contact, material) else {
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

    fn notify_pair(&self, lua: &Lua, hook: &str, (a, b): (u32, u32)) {
        for (id, other) in [(a, b), (b, a)] {
            for key in self.awoken_keys_for(id) {
                self.call_hook(lua, key, hook, (id, other));
            }
        }
    }
}

pub(super) fn material_of(scene: &rusty_scene::Scene, other: u32) -> Option<String> {
    let collider = scene.world.collider(other)?;
    rusty_scene::physics_material::named(&collider)
}

fn contact_table(lua: &Lua, c: &Contact, material: Option<String>) -> mlua::Result<Table> {
    let vec = |v: Vec3| -> mlua::Result<Table> {
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
    t.set("material", material)?;
    Ok(t)
}
