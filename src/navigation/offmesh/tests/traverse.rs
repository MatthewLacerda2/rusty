//! An agent on a link (#462): it stops at the ladder's foot, waits for its script
//! while auto-traverse is off, and walks on once the link is completed.

use glam::Vec3;

use super::*;
use crate::components::NavMeshAgentComponent;
use crate::navigation::complete_off_mesh_link;

const DT: f32 = 1.0 / 60.0;
const DECK: Vec3 = Vec3::new(16.0, 4.0, 5.0);

/// The ladder level with an agent on the ground heading for the deck.
fn climber(auto: bool) -> (Scene, NavigationGraph, u32) {
    let (mut scene, _) = ladder();
    let g = baked(&scene);
    let id = scene.add_entity("agent".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(2.0, 0.0, 5.0);
    let agent = NavMeshAgentComponent {
        active: true,
        target: DECK,
        speed: 4.0,
        acceleration: 20.0,
        stopping_distance: 0.3,
        auto_traverse_off_mesh_link: auto,
        ..Default::default()
    };
    scene.world.set_nav_agent(id, Some(agent));
    (scene, g, id)
}

fn pos(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

fn on_link(scene: &Scene, id: u32) -> bool {
    scene.world.nav_agent(id).unwrap().off_mesh_link.is_some()
}

#[test]
fn an_agent_waits_on_a_link_until_its_script_completes_it() {
    let (mut scene, g, id) = climber(false);
    let mut frames = 0;
    while !on_link(&scene, id) {
        g.tick_nav_agents(&mut scene, DT);
        frames += 1;
        assert!(
            frames < 600,
            "never reached the ladder: {}",
            pos(&scene, id)
        );
    }
    let foot = pos(&scene, id);
    assert!(foot.distance(Vec3::new(8.0, 0.0, 5.0)) < 0.6, "{foot}");
    for _ in 0..120 {
        g.tick_nav_agents(&mut scene, DT);
    }
    assert_eq!(pos(&scene, id), foot, "it waits for the script");
    assert!(on_link(&scene, id));

    let top = complete_off_mesh_link(&mut scene.world.nav_agent_mut(id).unwrap());
    scene.world.transform_mut(id).unwrap().position = top.unwrap();
    for _ in 0..600 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let end = pos(&scene, id);
    assert!(end.distance(DECK) < 0.5, "walks on to the deck: {end}");
}

#[test]
fn auto_traverse_climbs_by_itself_and_replays_bit_for_bit() {
    let run = || {
        let (mut scene, g, id) = climber(true);
        let mut was_on = false;
        for _ in 0..900 {
            g.tick_nav_agents(&mut scene, DT);
            was_on |= on_link(&scene, id);
        }
        assert!(was_on, "it crossed the link");
        pos(&scene, id).to_array().map(f32::to_bits)
    };
    let end = run();
    assert!(Vec3::from_array(end.map(f32::from_bits)).distance(DECK) < 0.5);
    assert_eq!(end, run(), "deterministic");
}
