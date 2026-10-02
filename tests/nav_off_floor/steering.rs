//! The navigation tick alone, on the default scene's bake: Enemy_1 retargeted to
//! the Player's position every frame, as `bot.lua` does.

use glam::Vec3;
use rusty::navigation::{is_at_target, NavigationGraph};
use rusty::scene::Scene;

use super::{assert_on_floor, default_scene, planar, FLOOR_HALF};

const DT: f32 = 1.0 / 60.0;

/// Chase `player` for `ticks` frames, checking the agent stands on the floor every
/// frame; returns where it ends.
fn chase(scene: &mut Scene, graph: &NavigationGraph, enemy: u32, player: Vec3, ticks: u32) -> Vec3 {
    for frame in 0..ticks {
        scene.world.nav_agent_mut(enemy).unwrap().target = player;
        graph.tick_nav_agents(scene, DT);
        let pos = scene.world.transform(enemy).unwrap().position;
        assert_on_floor(pos, &format!("chasing {player} (frame {frame})"));
    }
    scene.world.transform(enemy).unwrap().position
}

/// Stopped dead, and arrived by the agent's own measure.
fn assert_stopped(scene: &Scene, enemy: u32, pos: Vec3, when: &str) {
    let agent = scene.world.nav_agent(enemy).unwrap();
    assert_eq!(agent.velocity, Vec3::ZERO, "{when}: still pushing at {pos}");
    assert!(is_at_target(&agent, pos), "{when}: not arrived at {pos}");
}

#[test]
fn bot_stops_at_the_floor_edge_and_follows_the_player_back() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);

    // The Player flies 6 m past the +x edge, 1.5 m up.
    let off = chase(&mut scene, &graph, enemy, Vec3::new(25.0, 1.5, 8.0), 600);
    assert!(off.x > FLOOR_HALF - 1.5, "stopped short of the edge: {off}");
    assert!(
        (off.z - 8.0).abs() < 0.6,
        "the edge point nearest the Player: {off}"
    );
    assert_stopped(&scene, enemy, off, "at the edge");

    // Back on the floor: the bot follows and arrives under the flying Player.
    let home = Vec3::new(0.0, 1.5, -6.0);
    let back = chase(&mut scene, &graph, enemy, home, 900);
    assert!(planar(back, home) < 1.0, "did not follow back: {back}");
    assert_stopped(&scene, enemy, back, "back under the Player");
}

#[test]
fn bot_walks_along_the_edge_instead_of_pushing_into_it() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);
    scene.world.transform_mut(enemy).unwrap().position = Vec3::new(15.0, 1.05, 15.0);

    // Straight at the Player leaves the floor at its +x edge near z = 6; the bot
    // must instead walk to the edge point nearest the Player, far down the edge.
    let end = chase(&mut scene, &graph, enemy, Vec3::new(25.0, 1.5, -12.0), 900);
    assert!(end.x > FLOOR_HALF - 1.5, "left the edge: {end}");
    assert!(
        (end.z + 12.0).abs() < 0.6,
        "jammed partway along the edge: {end}"
    );
    assert_stopped(&scene, enemy, end, "down the edge");
}

#[test]
fn a_player_far_from_any_floor_leaves_the_bot_standing() {
    let (mut scene, enemy) = default_scene();
    let graph = NavigationGraph::from_scene(&scene);
    let start = scene.world.transform(enemy).unwrap().position;
    let end = chase(&mut scene, &graph, enemy, Vec3::new(60.0, 1.5, 8.0), 120);
    assert!(
        planar(start, end) < 1e-4,
        "walked toward an unreachable Player"
    );
}
