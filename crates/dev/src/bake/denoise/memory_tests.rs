//! OIDN's share of a lightmap bake's memory (#2312): held to its budgeted cost
//! (`budget::cost`) for one chart past the filter's scratch cap, and, with timing
//! moving how many charts it cleans at once, never growing with the scene. Each
//! test runs in a process of its own (`rusty_test_support::alone!`), so
//! the peak is this test's bakes'.

use super::{available, budget};
use crate::bake::lightmap::memory_tests::{bake, holds, DENSITY, MARGIN, SIDE, WORKERS};
use crate::memory::rss_peak_bytes;
use rusty_scene::lighting::settings::Denoiser;

/// Whether this machine can measure it: a readable peak and OIDN (whose absence
/// `RUSTY_REQUIRE_OIDN`'s canary fails on CI).
fn measurable() -> bool {
    rss_peak_bytes().is_some() && available().is_ok()
}

/// Texels of a square plane of `side` units at [`DENSITY`].
fn texels(side: f32) -> usize {
    let edge = (DENSITY * side) as usize;
    edge * edge
}

#[test]
fn one_chart_past_the_scratch_cap_costs_oidn_at_most_its_budgeted_cost() {
    // Checked before `alone!`, so a machine without OIDN starts no child.
    if !measurable() {
        return;
    }
    rusty_test_support::alone!(|| {
        // A 1024² chart: untiled, OIDN's scratch alone was about 1.1 GB a filter, and
        // both filters stayed on the device for the rest of the bake.
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
    // Checked before `alone!`, so a machine without OIDN starts no child.
    if !measurable() {
        return;
    }
    rusty_test_support::alone!(|| {
        let filtered = bake(2, SIDE, Denoiser::Filter);
        let share = (WORKERS as u64 * budget::cost(texels(SIDE))).min(budget::BUDGET);
        // Timing moves how many charts are denoised at once, so the 4× bake is held
        // to the Gaussian's peak with OIDN's whole budgeted share on top.
        let four = bake(8, SIDE, Denoiser::Oidn);
        holds((filtered.0 + share, filtered.1), four, 6, "OIDN");
    });
}
