//! The whole frame through `GameWorld` — `bot.lua`, the CharacterController's
//! physics body and the navigation tick together — so nothing downstream of the nav
//! move (defect 5 in #666: physics overriding it) sinks or blocks the bot, on the
//! deck or down the pool's ramps (#747).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scripting::ConsoleLogs;

use super::{
    assert_grounded, assert_in_pool_only_by_ramp, default_scene, home, planar, pool_middle, reach,
    INNER, WALL_MARGIN,
};

const DT: f32 = 1.0 / 60.0;

/// Play `ticks` frames with the Player pinned at `player` (re-placed every frame, so
/// it hovers); returns where Enemy_1 ends.
fn play(world: &mut GameWorld, player_id: u32, enemy: u32, player: Vec3, ticks: u32) -> Vec3 {
    for frame in 0..ticks {
        let scene = Rc::clone(world.scene());
        scene
            .borrow_mut()
            .world
            .transform_mut(player_id)
            .unwrap()
            .position = player;
        world.tick(DT);
        let pos = scene.borrow().world.transform(enemy).unwrap().position;
        let when = format!("playing toward {player} (frame {frame})");
        assert_grounded(pos, &when);
        assert_in_pool_only_by_ramp(pos, &when);
    }
    let scene = world.scene().borrow();
    let pos = scene.world.transform(enemy).unwrap().position;
    pos
}

#[test]
fn default_scene_bot_stays_in_the_yard_and_follows_through_the_pool() {
    rusty::scene::seed_default_scripts();
    let (scene, enemy) = default_scene();
    let reach = reach(&scene, enemy);
    let player_id = scene.find_entity_by_name("Player").expect("Player");
    let nav = NavigationGraph::from_scene(&scene);
    let mut world = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    world.set_playing(true);

    let off = play(&mut world, player_id, enemy, Vec3::new(17.0, 1.5, 8.0), 600);
    assert!(
        off.x > INNER.0 - WALL_MARGIN - 0.75 - reach,
        "stopped short of the wall: {off}"
    );

    let down = play(&mut world, player_id, enemy, pool_middle(), 900);
    assert!(
        planar(down, pool_middle()) < reach + 0.1,
        "never got down into the pool: {down}"
    );
    let back = play(&mut world, player_id, enemy, home(), 900);
    assert!(
        planar(back, home()) < reach + 0.1,
        "could not climb back out: {back}"
    );
}
