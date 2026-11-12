//! Answers that hold only where they were planned (#2087): a mask changed while
//! its search runs, a capped partial path walked out, a start off the navmesh.

use glam::Vec3;

use super::super::super::test_support::{add_box, add_floor};
use super::super::super::{NavBounds, NavigationGraph, ALL_AREAS};
use super::{agent, level, spawn, sync_path, DT};
use crate::components::NavPathStatus;
use crate::scene::authoring::nav::agent::set_area_mask;
use crate::scene::Scene;

#[test]
fn a_mask_changed_mid_search_plans_with_the_new_mask() {
    let mut g = level();
    let (from, to) = (Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 28.0));
    // Forbid the side of the block the unmasked path takes.
    let all = sync_path(&g, from, to);
    let left = all.iter().map(|p| p.x).sum::<f32>() < 15.0 * all.len() as f32;
    for r in g.span_refs().collect::<Vec<_>>() {
        if (g.span_world(r).x < 10.0) == left && g.span_world(r).z > 4.0 {
            g.spans[r.index as usize].area = 5;
        }
    }
    let mask = ALL_AREAS & !(1 << 5);
    let mut scene = Scene::new();
    let id = spawn(&mut scene, from, to);
    g.set_pathfinding_iterations_per_frame(7);
    g.tick_nav_agents(&mut scene, DT);
    assert!(g.requests.borrow().active.is_some(), "mid-flight");
    set_area_mask(&mut scene.world.nav_agent_mut(id).expect("agent"), mask);
    for _ in 0..2000 {
        g.tick_nav_agents(&mut scene, DT);
        if agent(&scene, id).path_planned {
            break;
        }
    }
    let a = agent(&scene, id);
    let goal = g.sample_position(to, 8.0).expect("goal");
    let masked = g.calculate_path_masked(from, goal, mask);
    let want: Vec<Vec3> = masked.corners.into_iter().skip(1).collect();
    assert!(
        a.path_planned && a.cached_path == want,
        "{:?}",
        a.cached_path
    );
    assert_ne!(a.cached_path, all, "the forbidden side is avoided");
}

/// A 40 × 40 floor at the 0.25 m default cell, a 2 m crate no step climbs, and a
/// wall between the crate and the agent.
fn crate_scene() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -1.0, 41.0, -1.0, 41.0);
    add_box(
        &mut scene,
        Vec3::new(30.0, 0.0, 30.0),
        Vec3::new(36.0, 2.0, 36.0),
    );
    // A wall the first capped search floods in front of, its gap at the far end.
    add_box(
        &mut scene,
        Vec3::new(-1.0, 0.0, 20.0),
        Vec3::new(35.0, 2.0, 21.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 40.0, 0.0, 40.0));
    NavigationGraph::from_scene(&scene)
}

#[test]
fn a_capped_path_is_asked_again_until_the_crates_foot() {
    let g = crate_scene();
    let mut scene = Scene::new();
    let top = Vec3::new(33.0, 2.0, 33.0);
    let id = spawn(&mut scene, Vec3::new(2.0, 0.0, 2.0), top);
    while !agent(&scene, id).path_planned {
        g.tick_nav_agents(&mut scene, DT);
    }
    let first = *agent(&scene, id)
        .cached_path
        .last()
        .expect("a partial path");
    assert!(
        agent(&scene, id).replan_from.is_some(),
        "the cap stopped it"
    );
    for _ in 0..1800 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let at = scene.world.world_position(id);
    let flat = |p: Vec3| Vec3::new(p.x, 0.0, p.z);
    assert!(
        flat(at).distance(flat(top)) < flat(first).distance(flat(top)) - 5.0,
        "past the first cap's end {first}: {at}"
    );
    assert!(at.x > 28.0 && at.z > 28.0, "at the crate's foot: {at}");
    let asked = g.requests.borrow().issued();
    for _ in 0..120 {
        g.tick_nav_agents(&mut scene, DT);
    }
    assert!(
        g.requests.borrow().issued() - asked <= 1,
        "no ask every tick"
    );
    assert_eq!(agent(&scene, id).path_status, NavPathStatus::Partial);
}

#[test]
fn an_agent_off_the_navmesh_asks_again_once_back_on_it() {
    // An open 30 × 30 floor with its 13 × 13 corner gone: nothing to snap to there.
    let mut g = NavigationGraph::new(0.0, 30.0, 0.0, 30.0, 1.0);
    for gz in (0..13).rev() {
        for gx in (0..13).rev() {
            let hole = g.index(gx, gz);
            g.spans.remove(g.cell_start[hole] as usize);
            g.cell_start[hole + 1..].iter_mut().for_each(|s| *s -= 1);
        }
    }
    let mut scene = Scene::new();
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(20.0, 0.0, 28.0),
    );
    g.tick_nav_agents(&mut scene, DT);
    let asked = g.requests.borrow().issued();
    for _ in 0..10 {
        g.tick_nav_agents(&mut scene, DT);
    }
    assert_eq!(agent(&scene, id).path_status, NavPathStatus::Invalid);
    assert_eq!(g.requests.borrow().issued(), asked, "not every tick");
    scene.world.transform_mut(id).expect("transform").position = Vec3::new(20.0, 0.0, 20.0);
    g.tick_nav_agents(&mut scene, DT);
    let a = agent(&scene, id);
    assert_eq!(a.path_status, NavPathStatus::Complete, "carried back on");
    assert!(a.replan_from.is_none());
}
