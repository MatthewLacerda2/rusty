//! The navigation tick alone, on the default scene's bake: Enemy_1 retargeted to
//! the Player's position every frame, as `bot.lua` does.

use glam::Vec3;
use rusty::navigation::{is_at_target, NavigationGraph};
use rusty::scene::Scene;

use super::{assert_grounded, default_scene, home, planar, reach, BASE_OFFSET, INNER, WALL_MARGIN};

const DT: f32 = 1.0 / 60.0;

/// Chase `player` for `ticks` frames, checking the agent stands on the ground every
/// frame and calling `each` with its position; returns where it ends.
pub fn chase_with(
    scene: &mut Scene,
    graph: &NavigationGraph,
    (enemy, player): (u32, Vec3),
    ticks: u32,
    mut each: impl FnMut(Vec3, &str),
) -> Vec3 {
    for frame in 0..ticks {
        scene.world.nav_agent_mut(enemy).unwrap().target = player;
        graph.tick_nav_agents(scene, DT);
        let pos = scene.world.transform(enemy).unwrap().position;
        let when = format!("chasing {player} (frame {frame})");
        assert_grounded(pos, &when);
        each(pos, &when);
    }
    scene.world.transform(enemy).unwrap().position
}

fn chase(scene: &mut Scene, graph: &NavigationGraph, enemy: u32, player: Vec3, ticks: u32) -> Vec3 {
    chase_with(scene, graph, (enemy, player), ticks, |_, _| {})
}

/// At rest, and arrived by the agent's own measure.
pub fn assert_stopped(scene: &Scene, enemy: u32, pos: Vec3, when: &str) {
    let agent = scene.world.nav_agent(enemy).unwrap();
    assert_eq!(agent.velocity, Vec3::ZERO, "{when}: still pushing at {pos}");
    assert!(is_at_target(&agent, pos), "{when}: not arrived at {pos}");
}

#[test]
fn bot_stops_at_the_wall_and_follows_the_player_back() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);

    // The Player flies 2 m past the +x wall, 1.5 m up.
    let off = chase(&mut scene, &graph, enemy, Vec3::new(17.0, 1.5, 8.0), 600);
    let reach = reach(&scene, enemy);
    assert!(
        off.x > INNER.0 - WALL_MARGIN - 0.75 - reach,
        "stopped short of the wall: {off}"
    );
    // It rests one stopping distance short of the last walkable cell's centre
    // nearest the Player, whichever way it came in.
    let nearest = Vec3::new(INNER.0 - WALL_MARGIN - 0.5, 0.0, 8.0);
    assert!(
        planar(off, nearest) < reach + 0.1,
        "the wall point nearest the Player: {off}"
    );
    assert_stopped(&scene, enemy, off, "at the wall");

    // Back over the yard: the bot follows and arrives under the flying Player.
    let back = chase(&mut scene, &graph, enemy, home(), 900);
    assert!(
        planar(back, home()) < reach + 0.1,
        "did not follow back: {back}"
    );
    assert_stopped(&scene, enemy, back, "back under the Player");
}

#[test]
fn bot_walks_along_the_wall_instead_of_pushing_into_it() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);
    scene.world.transform_mut(enemy).unwrap().position = Vec3::new(12.5, BASE_OFFSET, 15.0);

    // Straight at the Player meets the +x wall near z = 10; the bot must instead
    // walk to the wall point nearest the Player, far down the wall.
    let end = chase(&mut scene, &graph, enemy, Vec3::new(17.0, 1.5, -12.0), 900);
    let reach = reach(&scene, enemy);
    assert!(end.x > INNER.0 - WALL_MARGIN - 1.5, "left the wall: {end}");
    assert!(
        (end.z + 12.0).abs() < reach + 0.1,
        "jammed partway along the wall: {end}"
    );
    assert_stopped(&scene, enemy, end, "down the wall");
}

#[test]
fn a_player_far_outside_the_yard_leaves_the_bot_standing() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);
    let start = Vec3::new(8.0, BASE_OFFSET, 8.0); // standing on the deck
    scene.world.transform_mut(enemy).unwrap().position = start;
    let end = chase(&mut scene, &graph, enemy, Vec3::new(60.0, 1.5, 8.0), 120);
    assert!(
        planar(start, end) < 1e-4,
        "walked toward an unreachable Player"
    );
}
