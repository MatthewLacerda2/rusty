use std::collections::BTreeSet;

use proptest::prelude::*;

use super::lifecycle::{manager_with_entity, write_script};
use crate::scripting::manager::ScriptManager;
use rusty_components::ScriptComponent;
use rusty_ecs::changes::ops::{self, apply};

#[derive(Clone, Debug)]
enum Op {
    World(ops::Op),
    Attach(usize),
    Destroy(usize),
    Drain,
    Restart,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => rusty_ecs::edit_ops!().prop_map(Op::World),
        4 => (0usize..10).prop_map(Op::Attach),
        1 => (0usize..10).prop_map(Op::Destroy),
        3 => Just(Op::Drain),
        1 => Just(Op::Restart),
    ]
}

fn unloaded(m: &ScriptManager) -> BTreeSet<(u32, usize)> {
    let scene = m.cells.scene.borrow();
    let w = &scene.world;
    let slots = w.ids().iter().flat_map(|&id| {
        let n = w.scripts(id).map_or(0, |s| s.len());
        (0..n).map(move |i| (id, i))
    });
    slots.filter(|k| !m.load_attempted.contains(k)).collect()
}

fn drain(m: &mut ScriptManager) -> Result<(), TestCaseError> {
    let want = unloaded(m);
    let before = m.load_attempted.clone();
    m.load_new_scripts();
    let loaded: BTreeSet<_> = m.load_attempted.difference(&before).copied().collect();
    prop_assert_eq!(loaded, want);
    Ok(())
}

fn run(m: &mut ScriptManager, path: &str, op: &Op) -> Result<(), TestCaseError> {
    let pick = |m: &ScriptManager, i: usize| m.cells.scene.borrow().world.ids().get(i).copied();
    match op {
        Op::World(op) => apply(&mut m.cells.scene.borrow_mut().world, op),
        Op::Attach(i) => {
            if let Some(id) = pick(m, *i) {
                let script = ScriptComponent {
                    path: path.to_string(),
                    ..Default::default()
                };
                let mut scene = m.cells.scene.borrow_mut();
                scene.world.scripts_mut(id).expect("live").push(script);
            }
        }
        Op::Destroy(i) => {
            if let Some(id) = pick(m, *i) {
                m.cells.scene.borrow_mut().request_destroy(id);
                m.apply_pending_destroys();
            }
        }
        Op::Drain => drain(m)?,
        Op::Restart => m.init_runtime().expect("fresh VM"),
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn the_queue_loads_what_the_scan_would(ops in proptest::collection::vec(op(), 1..40)) {
        let (mut m, _) = manager_with_entity();
        m.init_runtime().expect("VM");
        let path = write_script("spawn_queue", "return {}");
        for op in &ops {
            run(&mut m, &path, op)?;
        }
        drain(&mut m)?;
    }
}
