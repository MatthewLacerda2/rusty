//! src/api/nav/areas.rs — navigation areas on the `Navigation` and `NavMeshAgent`
//! namespaces (#460).
//!
//! `Navigation`: the scene's area table — `GetAreaFromName` (Unity's
//! `NavMesh.GetAreaFromName`, `-1` when undefined), `DefineArea` (add a row or
//! re-cost one), `GetAreaCost` / `SetAreaCost` (by name, as Unity's
//! `NavMesh.SetAreaCost` by index) and `GetAreas`. A cost change writes the scene's
//! table and the live graph at once: the next search reads it, every cached agent
//! path re-plans, and nothing is rebaked. `NavMeshAgent`: `GetAreaMask` /
//! `SetAreaMask` (Unity's `areaMask`, bit `i` = area `i`; `-1` is every area).

use std::cell::RefCell;

use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::navigation::{area_index, NavMeshSettings, NavigationGraph};
use crate::scene::authoring::nav::agent as nav_ops;
use crate::scene::Scene;

/// Apply `edit` to the scene's area table and hand the new costs to the graph,
/// or raise `err` when the edit refuses.
fn edit_areas(
    scene: &RefCell<Scene>,
    nav: &RefCell<NavigationGraph>,
    edit: impl FnOnce(&mut NavMeshSettings) -> Option<u8>,
    err: impl FnOnce() -> String,
) -> mlua::Result<u8> {
    let index = edit(&mut scene.borrow_mut().nav_settings);
    let index = index.ok_or_else(|| mlua::Error::RuntimeError(err()))?;
    nav.borrow_mut()
        .set_area_costs(&scene.borrow().nav_settings.areas);
    Ok(index)
}

/// The area table functions on `Navigation`.
pub(super) fn register_navigation(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    let f = lua.create_function(move |lua, name: String| {
        scene.with(lua, |scene| {
            let index = area_index(&scene.borrow().nav_settings.areas, &name);
            Ok(index.map_or(-1, i32::from))
        })
    });
    put(t, "GetAreaFromName", f)?;
    let f = lua.create_function(move |lua, name: String| {
        let scene = &*scene.get(lua)?;
        let areas = &scene.borrow().nav_settings.areas;
        let row = area_index(areas, &name).map(|i| &areas[i as usize]);
        Ok(row.map(|a| a.cost))
    });
    put(t, "GetAreaCost", f)?;
    let f = lua.create_function(move |lua, (name, cost): (String, f32)| {
        let err =
            || format!("Navigation.SetAreaCost: no area {name:?} (or cost {cost} is not finite)");
        let (scene, nav) = (scene.get(lua)?, nav.get(lua)?);
        edit_areas(&scene, &nav, |s| s.set_area_cost(&name, cost), err).map(|_| ())
    });
    put(t, "SetAreaCost", f)?;
    let f = lua.create_function(move |lua, (name, cost): (String, Option<f32>)| {
        let cost = cost.unwrap_or(1.0);
        let err = || format!("Navigation.DefineArea: cannot define {name:?} (empty name, non-finite cost, or 32 areas already)");
        edit_areas(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.define_area(&name, cost), err)
    });
    put(t, "DefineArea", f)?;
    let f = lua.create_function(move |lua, ()| {
        let (list, scene) = (lua.create_table()?, scene.get(lua)?);
        for (i, a) in scene.borrow().nav_settings.areas.iter().enumerate() {
            let row = lua.create_table()?;
            row.set("index", i)?;
            row.set("name", a.name.as_str())?;
            row.set("cost", a.cost)?;
            list.set(i + 1, row)?;
        }
        Ok(list)
    });
    put(t, "GetAreas", f)
}

/// `GetAreaMask` / `SetAreaMask` on `NavMeshAgent`.
pub(super) fn register_agent(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let mask = scene
            .borrow()
            .world
            .nav_agent(id.read())
            .map(|a| a.area_mask);
        Ok(i64::from(mask.unwrap_or(0)))
    });
    put(t, "GetAreaMask", f)?;
    let f = lua.create_function(move |lua, (id, mask): (EntityArg, i64)| {
        let id = id.live("NavMeshAgent", "SetAreaMask")?;
        if let Some(mut a) = scene.get(lua)?.borrow_mut().world.nav_agent_mut(id) {
            nav_ops::set_area_mask(&mut a, mask as u32);
        }
        Ok(())
    });
    put(t, "SetAreaMask", f)
}

/// A query's optional area mask: every area when omitted.
pub(super) fn mask_arg(mask: Option<i64>) -> u32 {
    mask.map_or(u32::MAX, |m| m as u32)
}
