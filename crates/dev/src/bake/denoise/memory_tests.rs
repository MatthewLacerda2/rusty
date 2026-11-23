use super::{available, budget};
use crate::bake::lightmap::memory_tests::{bake, holds, DENSITY, MARGIN, SIDE, WORKERS};
use crate::memory::rss_peak_bytes;
use rusty_scene::lighting::settings::Denoiser;

/// Asked before `alone!`, so a machine that cannot measure starts no child process.
fn measurable() -> bool {
    rss_peak_bytes().is_some() && available().is_ok()
}

fn texels(side: f32) -> usize {
    let edge = (DENSITY * side) as usize;
    edge * edge
}

#[test]
fn one_chart_past_the_scratch_cap_costs_oidn_at_most_its_budgeted_cost() {
    if !measurable() {
        return;
    }
    rusty_test_support::alone!(|| {
        let side = 1024.0 / DENSITY;
        let filtered = bake(1, side, Denoiser::Filter);
        let cleaned = bake(1, side, Denoiser::Oidn);
        let (added, bound) = (
            cleaned.0.saturating_sub(filtered.0),
            budget::cost(texels(side)),
        );
        assert!(
            added <= bound + MARGIN,
            "OIDN added {} MB over the Gaussian's peak, past its {} MB cost",
            added >> 20,
            bound >> 20
        );
    });
}

#[test]
fn four_times_the_meshes_add_only_their_lightmaps_past_oidn_s_share() {
    if !measurable() {
        return;
    }
    rusty_test_support::alone!(|| {
        let filtered = bake(2, SIDE, Denoiser::Filter);
        let share = (WORKERS as u64 * budget::cost(texels(SIDE))).min(budget::BUDGET);
        let four = bake(8, SIDE, Denoiser::Oidn);
        holds((filtered.0 + share, filtered.1), four, "OIDN");
    });
}
