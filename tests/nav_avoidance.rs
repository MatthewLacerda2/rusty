//! Local avoidance between NavMesh agents (#463), driven through the public
//! `tick_nav_agents` tick on an open field: a head-on pair passes without
//! overlapping, priority decides who yields, and a non-avoiding agent is steered
//! around. Signed offsets, not only distances, so a flipped sign fails.

use glam::Vec3;
use rusty::navigation::NavigationGraph;
use rusty::scene::{NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.5;

fn agent(target: Vec3, priority: u8) -> NavMeshAgentComponent {
    NavMeshAgentComponent {
        active: true,
        radius: RADIUS,
        target,
        speed: 3.0,
        acceleration: 8.0,
        stopping_distance: 0.2,
        avoidance_priority: priority,
        ..Default::default()
    }
}

fn spawn(scene: &mut Scene, pos: Vec3, agent: NavMeshAgentComponent) -> u32 {
    let id = scene.add_entity("agent".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_nav_agent(id, Some(agent));
    id
}

fn pos(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

/// What a head-on crossing along z = 10 looked like.
struct Crossing {
    min_gap: f32,
    /// Signed z offset of each agent from the lane, at its widest.
    a_swerve: f32,
    b_swerve: f32,
    a_end: Vec3,
    b_end: Vec3,
}

/// A walks +x, B walks −x, both on the z = 10 lane.
fn cross(a: NavMeshAgentComponent, b: NavMeshAgentComponent) -> Crossing {
    let mut scene = Scene::new();
    let ia = spawn(&mut scene, Vec3::new(2.0, 0.0, 10.0), a);
    let ib = spawn(&mut scene, Vec3::new(18.0, 0.0, 10.0), b);
    let graph = NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0);
    let widest = |acc: f32, z: f32| {
        if (z - 10.0).abs() > acc.abs() {
            z - 10.0
        } else {
            acc
        }
    };
    let (mut min_gap, mut a_swerve, mut b_swerve) = (f32::MAX, 0.0f32, 0.0f32);
    for _ in 0..900 {
        graph.tick_nav_agents(&mut scene, DT);
        let (pa, pb) = (pos(&scene, ia), pos(&scene, ib));
        min_gap = min_gap.min(pa.distance(pb));
        a_swerve = widest(a_swerve, pa.z);
        b_swerve = widest(b_swerve, pb.z);
    }
    Crossing {
        min_gap,
        a_swerve,
        b_swerve,
        a_end: pos(&scene, ia),
        b_end: pos(&scene, ib),
    }
}

fn assert_arrived(end: Vec3, goal: Vec3) {
    let d = Vec3::new(end.x - goal.x, 0.0, end.z - goal.z).length();
    assert!(d < 0.6, "arrived at {goal:?}, ended at {end:?}");
}

#[test]
fn head_on_pair_passes_without_overlapping() {
    let (ga, gb) = (Vec3::new(18.0, 0.0, 10.0), Vec3::new(2.0, 0.0, 10.0));
    let c = cross(agent(ga, 50), agent(gb, 50));
    assert!(
        c.min_gap >= 2.0 * RADIUS * 0.95,
        "overlap: min gap {}",
        c.min_gap
    );
    // Equal priority: each keeps to its own right, so they swerve to opposite sides.
    assert!(c.a_swerve < -0.2, "A swerves toward -z: {}", c.a_swerve);
    assert!(c.b_swerve > 0.2, "B swerves toward +z: {}", c.b_swerve);
    assert_arrived(c.a_end, ga);
    assert_arrived(c.b_end, gb);
}

#[test]
fn lower_priority_agent_yields_to_the_higher() {
    let (ga, gb) = (Vec3::new(18.0, 0.0, 10.0), Vec3::new(2.0, 0.0, 10.0));
    let c = cross(agent(ga, 10), agent(gb, 90));
    assert!(
        c.min_gap >= 2.0 * RADIUS * 0.95,
        "overlap: min gap {}",
        c.min_gap
    );
    assert!(
        c.a_swerve.abs() < 0.05,
        "priority 10 holds its lane: {}",
        c.a_swerve
    );
    assert!(c.b_swerve > 0.5, "priority 90 steps aside: {}", c.b_swerve);
    assert_arrived(c.a_end, ga);
    assert_arrived(c.b_end, gb);
}

#[test]
fn agent_with_avoidance_off_is_steered_around_but_never_swerves() {
    let (ga, gb) = (Vec3::new(18.0, 0.0, 10.0), Vec3::new(2.0, 0.0, 10.0));
    let mut a = agent(ga, 50);
    a.avoidance_enabled = false;
    let c = cross(a, agent(gb, 50));
    assert!(
        c.min_gap >= 2.0 * RADIUS * 0.95,
        "overlap: min gap {}",
        c.min_gap
    );
    assert!(
        c.a_swerve.abs() < 0.05,
        "avoidance off holds its lane: {}",
        c.a_swerve
    );
    assert!(
        c.b_swerve > 0.5,
        "the avoiding agent takes the whole dodge: {}",
        c.b_swerve
    );
}
