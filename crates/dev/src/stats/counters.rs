//! crates/dev/src/stats/counters.rs — the world counters a frame's stats record at
//! its end: what the world holds, and what each change channel dirtied, read by
//! [`TimingProbe::record_frame`], the probe's own work (`probe_ms`).

use std::collections::BTreeMap;

use rusty_app::{Resources, Stage, World};
use rusty_core::frame_stats::FrameStats;

use super::{stage_key, TimingProbe};

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

impl TimingProbe {
    /// The frame's stage totals, its loads' samples and the world counters, into
    /// the stats: [`end_frame`](rusty_app::SystemProbe::end_frame)'s work, timed as the probe's own.
    pub(super) fn record_frame(&mut self, world: &World, res: &Resources) {
        #[cfg(test)]
        super::verbs::advance(RECORD_COST.with(std::cell::Cell::get));
        let mut stats = self.stats.borrow_mut();
        // The first frame of a Play session (`advance_frame` has just made it 1)
        // starts the stats afresh.
        if res.play_frame() == 1 {
            *stats = FrameStats::default();
        }
        for (key, ms) in self.loads.borrow_mut().drain(..) {
            stats.record(&key, ms);
        }
        for (stage, ms) in Stage::FRAME_ORDER.iter().zip(self.stage_ms) {
            stats.record(stage_key(*stage), ms);
        }
        self.stage_ms = [0.0; 4];
        record_world(&mut stats, world, res);
        let changes = world.scene.borrow().world.change_stats();
        let changes = changes.into_iter().map(|s| (s.name, s.logged));
        record_changes(&mut stats, changes, &mut self.logged);
        self.frame_ran = true;
    }
}

#[cfg(test)]
thread_local! {
    /// What each `record_frame` charges a test's driven clock, in ms: the probe's
    /// own work made visible (`probe_cost_tests`).
    pub(super) static RECORD_COST: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
