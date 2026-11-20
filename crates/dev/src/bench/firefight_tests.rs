//! The bench's firefight (`scenarios/bench/firefight.lua`, #2448) on a small
//! stress scene, stepped without rendering: it churns at a steady state (as many
//! entities and decals at the end of a window as at its start, and every
//! instance spawned and destroyed in between), hangs its instances from a hand
//! bone, and does the same edits on two runs.

use std::collections::BTreeMap;

use super::stress::{self, StressSpec};
use crate::harness::{fixture_project, Harness};
use rusty_components::ScriptFieldValue;

/// Ticks the firefight is given to reach its steady counts (the scenario's).
const WARMUP: u32 = 120;
/// What a window measures here: enough for every instance to turn over.
const WINDOW: u32 = 90;

/// A harness on four soldiers and a few decals, the firefight attached.
fn fight(name: &str) -> Harness {
    let out =
        rusty_test_support::temp::dir().join(format!("firefight_{name}_{}", std::process::id()));
    let h = Harness::new(out, "");
    let spec = StressSpec {
        enemies: 4,
        lights: 0,
        flickering: 0,
        shadowed: 0,
        decals: 8,
        particle_systems: 0,
        ..StressSpec::default()
    };
    {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        stress::load(&mut scene, &spec);
        let dir = fixture_project().join("scenarios/bench");
        let id = scene.add_entity("Firefight".to_string());
        let prefab = dir.join("casing.prefab").to_string_lossy().into_owned();
        let fields = BTreeMap::from([("prefab".to_string(), ScriptFieldValue::Text(prefab))]);
        let script = dir.join("firefight.lua").to_string_lossy().into_owned();
        assert!(rusty_edit::authoring::add_script(
            &mut scene.world,
            id,
            &script,
            fields
        ));
    }
    h
}

/// The scene's live entities, decals and casings (by name, the prefab root's).
fn counts(h: &Harness) -> (usize, usize, usize) {
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let casings = (scene.entity_ids().into_iter())
        .filter(|&id| scene.world.name(id).is_some_and(|n| n.as_str() == "Casing"))
        .count();
    (scene.entity_count(), scene.decals.len(), casings)
}

#[test]
fn the_firefight_churns_at_a_steady_state() {
    let h = fight("steady");
    let (quiet, quiet_decals, _) = counts(&h);
    h.step(WARMUP);
    let first = counts(&h);
    assert_eq!(
        first.2,
        4 * 30,
        "4 casings a tick, 30 ticks each: {first:?}"
    );
    assert_eq!(
        first.1,
        quiet_decals + 64,
        "64 of its decals kept: {first:?}"
    );
    // Muzzles, holders and the pickup, and the casings beside them.
    assert!(first.0 > quiet + first.2, "{first:?} vs {quiet}");
    let ids_before = h.world.borrow().scene().borrow().entity_ids();
    h.step(WINDOW);
    assert_eq!(
        counts(&h),
        first,
        "the window's last frame holds its first's counts"
    );
    let ids_after = h.world.borrow().scene().borrow().entity_ids();
    let kept = ids_after
        .iter()
        .filter(|id| ids_before.contains(id))
        .count();
    assert_eq!(ids_after.len() - kept, 4 * 30, "every casing turned over");
    // One casing a tick hangs from a soldier's hand bone.
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let on_hand = (scene.entity_ids().into_iter())
        .filter(|&id| scene.world.name(id).is_some_and(|n| n.as_str() == "Casing"))
        .filter_map(|id| scene.world.parent_id(id))
        .filter(|&p| scene.world.name(p).is_some_and(|n| n.as_str() == "hand_r"))
        .count();
    assert_eq!(on_hand, 30, "one live casing in four is on a hand");
    drop(scene);
    drop(world);
    let console = h.console_text();
    assert!(!console.contains("[ERROR]"), "{console}");
}

#[test]
fn two_firefights_do_the_same_edits() {
    let run = |name| {
        let h = fight(name);
        h.step(WARMUP + 10);
        let names = |h: &Harness| {
            let world = h.world.borrow();
            let scene = world.scene().borrow();
            let ids = scene.entity_ids();
            ids.iter()
                .map(|&id| (id, scene.world.name(id).map(|n| n.clone())))
                .collect::<Vec<_>>()
        };
        (counts(&h), names(&h), h.snapshot())
    };
    assert_eq!(run("a"), run("b"));
}
