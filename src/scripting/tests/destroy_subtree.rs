//! A destroy tears down what it removes (#1283): `Scene.DestroyEntity` takes the
//! entity's whole subtree (Unity's `Object.Destroy`), and every script on it gets
//! `OnDisable` then `OnDestroy`, children before parents, and leaves no instance,
//! timer or coroutine behind. One set, `Scene::destroy_set`, decides both.

use std::collections::BTreeMap;

use super::lifecycle::write_script;
use super::timers::{rig, tick};
use super::transitions::LOGGER;
use crate::scene::skeleton::fixture::armature_owner;
use crate::scripting::manager::ScriptManager;

/// `LOGGER`'s tags, plus a pending invoke and a waiting coroutine from `Start`.
const BUSY: &str = "_G.__log = _G.__log or ''\n\
    local M = {}\n\
    function M.Beat() end\n\
    function M.Start(id) Timer.Invoke(id, 'Beat', 10)\n\
      Timer.StartCoroutine(id, function() coroutine.yield(Timer.WaitForSeconds(10)) end) end\n\
    function M.OnDisable(id) __log = __log .. 'D' .. id end\n\
    function M.OnDestroy(id) __log = __log .. 'X' .. id end\n\
    return M";

/// A started runtime whose `parent` carries `LOGGER`; `child` (made by `under`
/// from the scene and the parent) carries `BUSY`. Returns the manager, parent, child.
pub(super) fn scripted(
    name: &str,
    under: impl FnOnce(&mut crate::scene::Scene, u32) -> u32,
) -> (ScriptManager, u32, u32) {
    let (mut m, parent) = rig(name, LOGGER);
    let child = under(&mut m.cells.scene.borrow_mut(), parent);
    let path = write_script(&format!("{name}_child"), BUSY);
    m.load_entity_script(child, 0, &path, &BTreeMap::new())
        .unwrap();
    tick(&mut m, 1);
    let stem = format!("rusty_lifecycle_{name}_child");
    let live = crate::scripting::instances::find(m.lua.as_ref().unwrap(), child, &stem);
    assert!(live.unwrap().is_some(), "the child's Lua table is indexed");
    assert!(
        !m.cells.timers.borrow().is_empty(),
        "the child's invoke and coroutine wait"
    );
    m.eval("__log = ''").unwrap();
    (m, parent, child)
}

/// Request `id`'s destroy and drain it, as the Play schedule does.
pub(super) fn destroy(m: &mut ScriptManager, id: u32) {
    m.cells.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
}

/// Nothing of `id` survives: no entity, script instance, Lua table (its script's
/// file stem is `stem`) or timer.
fn assert_forgotten(m: &ScriptManager, id: u32, stem: &str) {
    assert!(
        !m.cells.scene.borrow().world.contains(id),
        "entity {id} is despawned"
    );
    assert!(
        m.entity_scripts.keys().all(|&(e, _)| e != id),
        "no instance of {id}"
    );
    let lua = m.lua.as_ref().unwrap();
    let table = crate::scripting::instances::find(lua, id, stem).unwrap();
    assert!(table.is_none(), "no Lua table of {id}");
    assert!(
        m.cells.timers.borrow().is_empty(),
        "no timer or coroutine of {id}"
    );
}

/// A skinned rig (`parent` holds it) whose `head` bone carries the scripted child.
fn rig_with_scripted_gun(name: &str) -> (ScriptManager, u32, u32, Vec<u32>) {
    let mut bones = Vec::new();
    let (m, holder, gun) = scripted(name, |scene, holder| {
        let rig = armature_owner(scene);
        scene.set_parent(rig, Some(holder)).unwrap();
        scene.sync_skeleton(rig);
        bones = scene.world.mesh(rig).unwrap().skeleton.ids();
        let gun = scene.add_entity("Gun".to_string());
        scene.set_parent(gun, Some(bones[2])).unwrap();
        gun
    });
    (m, holder, gun, bones)
}

#[test]
fn destroying_a_rig_tears_down_the_script_on_its_bone() {
    let (mut m, holder, gun, _) = rig_with_scripted_gun("destroy_rig");
    let rig = m.cells.scene.borrow().world.children(holder)[0];
    destroy(&mut m, rig);
    assert_eq!(m.eval("__log").unwrap(), format!("D{gun}X{gun}"));
    assert_forgotten(&m, gun, "rusty_lifecycle_destroy_rig_child");
    assert!(
        m.cells.scene.borrow().world.contains(holder),
        "the rig's parent stays"
    );
}

#[test]
fn destroying_a_bone_tears_down_the_script_hanging_from_it() {
    let (mut m, _, gun, bones) = rig_with_scripted_gun("destroy_bone");
    destroy(&mut m, bones[1]); // spine, and head under it
    assert_eq!(m.eval("__log").unwrap(), format!("D{gun}X{gun}"));
    assert_forgotten(&m, gun, "rusty_lifecycle_destroy_bone_child");
}

#[test]
fn destroying_a_parent_takes_its_child_and_tears_it_down_first() {
    let (mut m, parent, child) = scripted("destroy_plain", |scene, parent| {
        let child = scene.add_entity("Child".to_string());
        scene.set_parent(child, Some(parent)).unwrap();
        child
    });
    destroy(&mut m, parent);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{child}D{parent}X{child}X{parent}"),
        "OnDisable then OnDestroy, each child before its parent"
    );
    assert_forgotten(&m, child, "rusty_lifecycle_destroy_plain_child");
    assert_forgotten(&m, parent, "rusty_lifecycle_destroy_plain");
}
