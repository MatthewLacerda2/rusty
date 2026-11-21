use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::callbacks::ON_CONTROLLER_COLLIDER_HIT;
use super::super::instances;
use super::super::manager::ScriptManager;
use rusty_physics::ControllerHit;
use rusty_scene::Scene;

#[derive(Clone, Default)]
struct Dispatching(Rc<RefCell<BTreeSet<u32>>>);

struct Held(Dispatching, u32);

impl Drop for Held {
    fn drop(&mut self) {
        self.0 .0.borrow_mut().remove(&self.1);
    }
}

fn enter(lua: &Lua, id: u32) -> Option<Held> {
    let existing = lua.app_data_ref::<Dispatching>().map(|d| d.clone());
    let set = existing.unwrap_or_else(|| {
        let d = Dispatching::default();
        lua.set_app_data(d.clone());
        d
    });
    let fresh = set.0.borrow_mut().insert(id);
    fresh.then_some(Held(set, id))
}

pub fn dispatch_controller_hits(
    lua: &Lua,
    scene: &RefCell<Scene>,
    id: u32,
    hits: &[ControllerHit],
) -> mlua::Result<()> {
    let active = {
        let s = scene.borrow();
        s.world.contains(id) && s.world.is_active_in_hierarchy(id)
    };
    if hits.is_empty() || !active {
        return Ok(());
    }
    let handlers = instances::handlers(lua, id, ON_CONTROLLER_COLLIDER_HIT)?;
    if handlers.is_empty() {
        return Ok(());
    }
    let Some(_held) = enter(lua, id) else {
        return Ok(());
    };
    for hit in hits {
        let table = hit_table(lua, &scene.borrow(), hit)?;
        for script in &handlers {
            let f: mlua::Function = script.get(ON_CONTROLLER_COLLIDER_HIT)?;
            f.call::<()>((id, table.clone()))?;
        }
    }
    Ok(())
}

impl ScriptManager {
    pub fn dispatch_queued_controller_hits(&self, queued: &[(u32, Vec<ControllerHit>)]) {
        if queued.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for (id, hits) in queued {
                if let Err(e) = dispatch_controller_hits(lua, &self.cells.scene, *id, hits) {
                    self.cells.console.borrow_mut().error(format!(
                        "[Lua Error] {ON_CONTROLLER_COLLIDER_HIT} on entity {id} failed: {e}"
                    ));
                }
            }
        });
    }
}

fn hit_table(lua: &Lua, scene: &Scene, hit: &ControllerHit) -> mlua::Result<Table> {
    let vec = |v: Vec3| -> mlua::Result<Table> {
        let t = lua.create_table()?;
        t.set("x", v.x)?;
        t.set("y", v.y)?;
        t.set("z", v.z)?;
        Ok(t)
    };
    let world = &scene.world;
    let rigidbody = world
        .body_carrier(hit.id)
        .filter(|&b| world.has_rigidbody(b) && !world.is_static(b));
    let t = lua.create_table()?;
    t.set("id", hit.id)?;
    t.set("root", scene.root_of(hit.id))?;
    t.set("rigidbody", rigidbody)?;
    t.set("point", vec(hit.point)?)?;
    t.set("normal", vec(hit.normal)?)?;
    t.set("moveDirection", vec(hit.move_direction)?)?;
    t.set("moveLength", hit.move_length)?;
    t.set("material", super::physics::material_of(scene, hit.id))?;
    Ok(t)
}
