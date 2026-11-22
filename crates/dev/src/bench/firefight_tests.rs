use std::collections::BTreeMap;

use super::stress::{self, StressSpec};
use crate::harness::{fixture_project, Harness};
use rusty_components::ScriptFieldValue;

const WARMUP: u32 = 120;
const WINDOW: u32 = 90;

pub(super) fn fight(name: &str, trace: bool) -> Harness {
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
        let fields = BTreeMap::from([
            ("prefab".to_string(), ScriptFieldValue::Text(prefab)),
            ("trace".to_string(), ScriptFieldValue::Boolean(trace)),
        ]);
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
    let h = fight("steady", false);
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
        let h = fight(name, false);
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
