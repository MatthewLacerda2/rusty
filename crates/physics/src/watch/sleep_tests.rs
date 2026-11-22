use super::ops_tests::{Op, Phys};
use super::tests::{agree, Twin};
use rusty_ecs::changes::ops::{Edit, Op as W};

fn id(twin: &Twin, i: usize) -> u32 {
    twin.a.world.ids()[i]
}

fn asleep(twin: &Twin, i: usize) -> bool {
    let p = &twin.fast;
    let body = p.id_to_body.get(&id(twin, i));
    body.is_some_and(|&h| p.bodies[h].is_sleeping())
}

fn run(twin: &mut Twin, op: &Op) {
    let ran = twin.run(op);
    assert!(ran.is_ok(), "{op:?}: {ran:?}");
}

#[test]
fn settling_puts_the_fixture_to_sleep() {
    let mut twin = Twin::new();
    run(&mut twin, &Op::Settle);
    for i in 1..4 {
        assert!(asleep(&twin, i), "body {i} still awake after settling");
    }
}

#[test]
fn a_sleeping_body_hit_by_a_moving_one_steps_as_the_full_pass() {
    let mut twin = Twin::new();
    run(&mut twin, &Op::Settle);
    assert!(asleep(&twin, 2), "the sphere must be asleep before the hit");
    let before = twin.a.world.transform(id(&twin, 2)).map(|t| t.position);
    run(&mut twin, &Op::On(1, Phys::Velocity(i8::MAX)));
    for _ in 0..9 {
        run(&mut twin, &Op::Settle);
    }
    let after = twin.a.world.transform(id(&twin, 2)).map(|t| t.position);
    assert_ne!(before, after, "the hit never reached the sleeping sphere");
}

#[test]
fn a_disabled_body_under_a_falling_one_is_written_back() {
    let ops = vec![
        Op::World(W::On(2, Edit::Adopt(false))),
        Op::World(W::On(5, Edit::Reparent(Some(1)))),
        Op::Step,
        Op::Pause,
    ];
    let r = agree(&ops);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn a_settled_body_moved_by_a_reparent_wakes() {
    let ops = vec![
        Op::Settle,
        Op::World(W::On(3, Edit::Reparent(Some(1)))),
        Op::On(1, Phys::Velocity(-60)),
        Op::Settle,
    ];
    let r = agree(&ops);
    assert!(r.is_ok(), "{r:?}");
}
