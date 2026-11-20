//! crates/dev/src/stats/counters.rs — the world counters a frame's stats record at
//! its end: what the world holds, and what each change channel dirtied.

use std::collections::BTreeMap;

use rusty_app::{Resources, World};
use rusty_core::frame_stats::FrameStats;

/// The world counters: entities, rigid bodies, nav agents, live particles, scripts,
/// skeletons and their bones (#1617).
pub(super) fn record_world(stats: &mut FrameStats, world: &World, res: &Resources) {
    let scene = world.scene.borrow();
    let w = &scene.world;
    let particles: usize = w
        .ids_with_particles()
        .into_iter()
        .filter_map(|id| w.particles(id).map(|p| p.live_count()))
        .sum();
    let skinned = scene.skinned_ids();
    let bones: usize = skinned
        .iter()
        .filter_map(|&id| {
            w.mesh(id)
                .map(|m| m.skeleton.bones.iter().flatten().count())
        })
        .sum();
    stats.record("entities", w.len() as f64);
    stats.record("rigid_bodies", w.ids_with_rigidbody().len() as f64);
    stats.record("nav_agents", w.ids_with_nav_agent().len() as f64);
    stats.record("particles", particles as f64);
    stats.record("scripts", res.script_manager.live_script_count() as f64);
    stats.record("skeletons", skinned.len() as f64);
    stats.record("bones", bones as f64);
}

/// `changes_<channel>`: per change channel a cursor reads, the distinct ids dirtied
/// this frame (the entries its log appended since the last frame, #1705).
pub(super) fn record_changes(
    stats: &mut FrameStats,
    channels: impl Iterator<Item = (&'static str, u64)>,
    last: &mut BTreeMap<&'static str, u64>,
) {
    for (name, logged) in channels {
        let before = last.insert(name, logged).unwrap_or(0);
        let key = format!("changes_{name}");
        stats.record(&key, logged.saturating_sub(before) as f64);
    }
}
