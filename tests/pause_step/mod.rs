mod control;
mod step_pump;

use rusty::dev::harness::Harness;
use rusty::time::FIXED_DELTA_TIME;

/// `GameWorld::paused_frame`'s step loop, copied: a change to that loop is made here too.
fn drain_steps(h: &Harness) -> u32 {
    let mut ran = 0;
    loop {
        let mut world = h.world.borrow_mut();
        let take = world.time().borrow_mut().take_pending_step();
        if !take {
            break;
        }
        world.tick(FIXED_DELTA_TIME);
        ran += 1;
    }
    ran
}

/// Read an entity's world position. The `let` binding is load-bearing: it drops the
/// transform guard before `scene` does, so the borrow checker is satisfied
/// (returning the field expression directly would outlive `scene`).
fn entity_pos(h: &Harness, id: u32) -> glam::Vec3 {
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    let pos = scene.world.transform(id).unwrap().position;
    drop(scene);
    pos
}

fn player_id(h: &Harness) -> u32 {
    let world = h.world.borrow();
    let scene = world.scene().borrow();
    scene.find_entity_by_name("Player").expect("Player exists")
}
