//! A two-storey building: a ground floor, an upper floor over half of it, and a
//! staircase between them.

use glam::Vec3;
use rusty::navigation::{NavBounds, NavigationGraph};
use rusty::scene::{NavMeshAgentComponent, Scene};

use super::{add_box, bake, heights};

/// Ground 0..20 square; upper floor (top y = 3) over x = 9.5..20; six 0.5-high
/// stair blocks over x = 3.5..9.5, z = 2..6, climbing from the ground to it.
pub fn building() -> (Scene, NavigationGraph) {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(20.0, 0.0, 20.0),
    );
    add_box(
        &mut scene,
        Vec3::new(9.5, 2.8, 0.0),
        Vec3::new(20.0, 3.0, 20.0),
    );
    for i in 0..6 {
        let x = 3.5 + i as f32;
        let top = 0.5 * (i + 1) as f32;
        add_box(
            &mut scene,
            Vec3::new(x, 0.0, 2.0),
            Vec3::new(x + 1.0, top, 6.0),
        );
    }
    let g = bake(&mut scene, NavBounds::new(0.0, 20.0, 0.0, 20.0));
    (scene, g)
}

#[test]
fn both_floors_are_walkable_at_once() {
    let (_, g) = building();
    let floors: Vec<f32> = g.spans_at(15, 10).iter().map(|s| s.y).collect();
    assert_eq!(
        floors,
        vec![0.0, 3.0],
        "ground under the upper floor, and the floor"
    );
    assert_eq!(g.spans_at(5, 10).len(), 1, "single storey outside it");
}

#[test]
fn path_climbs_the_stairs_to_the_upper_floor() {
    let (_, g) = building();
    let path = g
        .path_between(Vec3::new(2.0, 0.0, 10.0), Vec3::new(15.0, 3.0, 15.0))
        .expect("the stairs connect the floors");
    let ys = heights(&g, &path);
    assert_eq!(ys.first(), Some(&0.0));
    assert_eq!(ys.last(), Some(&3.0), "ends on the upper floor: {ys:?}");
    assert!(
        path.iter().any(|s| (4..=8).contains(&s.gx) && s.gz <= 5),
        "via the stairs"
    );
}

#[test]
fn ground_target_under_the_upper_floor_stays_on_the_ground() {
    let (_, g) = building();
    let path = g
        .path_between(Vec3::new(2.0, 0.0, 10.0), Vec3::new(15.0, 1.0, 15.0))
        .expect("the ground floor is one surface");
    assert!(heights(&g, &path).iter().all(|&y| y == 0.0), "never climbs");
}

#[test]
fn agent_walks_upstairs_deterministically() {
    let run = || {
        let (mut scene, g) = building();
        let id = scene.add_entity("agent".to_string());
        scene.world.transform_mut(id).expect("agent").position = Vec3::new(2.0, 0.0, 10.0);
        let agent = NavMeshAgentComponent {
            active: true,
            radius: 0.5,
            target: Vec3::new(15.0, 3.0, 15.0),
            speed: 4.0,
            acceleration: 10.0,
            stopping_distance: 0.5,
            ..Default::default()
        };
        scene.world.set_nav_agent(id, Some(agent));
        for _ in 0..1200 {
            g.tick_nav_agents(&mut scene, 1.0 / 60.0);
        }
        let pos = scene.world.transform(id).expect("agent").position;
        pos
    };
    let pos = run();
    assert_eq!(pos.y, 3.0, "the agent ends upstairs: {pos}");
    assert!(
        pos.distance(Vec3::new(15.0, 3.0, 15.0)) < 1.0,
        "at its target: {pos}"
    );
    assert_eq!(pos.to_array(), run().to_array(), "replay is byte-identical");
}
