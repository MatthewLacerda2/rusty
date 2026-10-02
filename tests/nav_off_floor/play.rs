//! The whole frame through `GameWorld` — `bot.lua`, the CharacterController's
//! physics body and the navigation tick together — so nothing downstream of the nav
//! move (defect 5 in #666: physics overriding it) sinks or blocks the bot.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scripting::ConsoleLogs;

use super::{assert_on_floor, default_scene, planar, FLOOR_HALF};

const DT: f32 = 1.0 / 60.0;

/// Play `ticks` frames with the Player pinned at `player` (flying: no gravity in the
/// default scene); returns where Enemy_1 ends.
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
        assert_on_floor(pos, &format!("playing toward {player} (frame {frame})"));
    }
    let scene = world.scene().borrow();
    let pos = scene.world.transform(enemy).unwrap().position;
    pos
}

#[test]
fn default_scene_bot_stays_on_the_floor_when_the_player_flies_off_it() {
    rusty::scene::seed_default_scripts();
    let (scene, enemy) = default_scene();
    let player_id = scene.find_entity_by_name("Player").expect("Player");
    let nav = NavigationGraph::from_scene(&scene);
    let mut world = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    world.set_playing(true);

    let off = play(&mut world, player_id, enemy, Vec3::new(25.0, 1.5, 8.0), 600);
    assert!(off.x > FLOOR_HALF - 1.5, "stopped short of the edge: {off}");

    let home = Vec3::new(0.0, 1.5, -6.0);
    let back = play(&mut world, player_id, enemy, home, 900);
    assert!(planar(back, home) < 1.0, "could not climb back: {back}");
}
