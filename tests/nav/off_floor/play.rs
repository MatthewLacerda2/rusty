//! The whole frame through `GameWorld` — a script, the CharacterController's physics
//! body and the navigation tick together — so nothing downstream of the nav move
//! (defect 5 in #666: physics overriding it) sinks or blocks the bot, on the deck or
//! down the pool's ramps (#747). The chase there is a plain one (`CHASE`): the
//! default `bot.lua` stops at a spot round the Player an `EnvironmentQuery` picks
//! (#1892), and its own test holds that.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::script::scripting::ConsoleLogs;

use super::{
    assert_grounded, assert_in_pool_only_by_ramp, default_scene, default_scene_with, home, planar,
    pool_middle, reach, INNER, WALL_MARGIN,
};

/// A brain that heads straight for the Player every frame.
const CHASE: &str = r#"
local Chase = {}
function Chase.Update(id, dt)
    local x, y, z = Transform.GetPosition(Scene.FindEntityByName("Player"))
    NavMeshAgent.SetTarget(id, x, y, z)
end
return Chase
"#;

/// A `GameWorld` playing `scene`.
fn world_of(scene: rusty::scene::Scene) -> GameWorld {
    let nav = NavigationGraph::from_scene(&scene);
    let mut world = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    world.set_playing(true);
    world
}

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
fn a_chasing_bot_stays_in_the_yard_and_follows_through_the_pool() {
    let script = crate::temp::file("chase.lua");
    std::fs::write(&script, CHASE).unwrap();
    let (scene, enemy) = default_scene_with(script.to_str().unwrap());
    let reach = reach(&scene, enemy);
    let player_id = scene.find_entity_by_name("Player").expect("Player");
    let mut world = world_of(scene);

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

#[test]
fn the_default_bot_holds_a_spot_round_the_player() {
    rusty::scene::seed_default_scripts();
    let (scene, enemy) = default_scene();
    let player_id = scene.find_entity_by_name("Player").expect("Player");
    let mut world = world_of(scene);
    // Its query rings the Player 3–8 m out; the agent stops within its
    // stopping distance of the spot.
    let at = play(&mut world, player_id, enemy, home(), 600);
    let d = planar(at, home());
    assert!((2.0..10.0).contains(&d), "{d} m from the Player: {at}");
}
