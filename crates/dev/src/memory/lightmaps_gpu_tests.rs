//! A level's lightmap pages hold staging only up to its bound too (#2372): each
//! page's write goes through the counted texture write
//! (`rusty_render::STAGING_BUDGET`), so four times the pages peak higher only by
//! their GPU copies, not by four times the pages in staging. The pages are made
//! before the measure, uploaded through `Renderer::upload_lightmap_pages` and
//! kept until the GPU copies are counted, so the peak is the upload's: where the
//! GPU copies are host memory (lavapipe), a page freed before its copy ran would
//! make room for it and hide what staging held. The measure is
//! `textures_gpu_tests`'.

use rusty_render::{Renderer, STAGING_BUDGET};

use super::textures_gpu_tests::{device_bytes, mb, set_up};
use crate::memory::rss_peak_bytes;

/// A page's edge: one level of `Rgba16Float`, 8 MiB.
const EDGE: u32 = 1024;
/// The smaller list; the larger is four times it.
const PAGES: usize = 4;
/// Slack on top of the bound: the allocator's and the driver's.
const MARGIN: u64 = 24 << 20;

/// `n` pages of one light, every byte touched so they are resident before the
/// measure starts.
fn pages(n: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|_| vec![0x3c; (EDGE * EDGE * 8) as usize])
        .collect()
}

/// Upload `pages`: the peak after, the GPU bytes they hold, and their texture and
/// pages (kept, so both stay live).
fn upload(renderer: &Renderer, pages: Vec<Vec<u8>>) -> (u64, u64, impl Sized) {
    let before = device_bytes(renderer);
    let texture = renderer.upload_lightmap_pages(EDGE, pages);
    let held = device_bytes(renderer) - before;
    (rss_peak_bytes().expect("read"), held, texture)
}

#[test]
fn gpu_four_times_the_lightmap_pages_peak_higher_only_by_their_gpu_copies() {
    rusty_test_support::alone!(|| {
        let Some((renderer, _dir)) = set_up("lightmap_staging") else {
            return;
        };
        let (few, many) = (pages(PAGES), pages(4 * PAGES));
        let one = upload(&renderer, few);
        let four = upload(&renderer, many);
        let grew = four.0.saturating_sub(one.0);
        let page = u64::from(EDGE * EDGE * 8);
        // In staging at once: the pages pending and those of the submit before,
        // each at most the budget plus the one page that crossed it.
        let bound = 2 * (STAGING_BUDGET + page) + MARGIN;
        println!(
            "{PAGES} then {} pages: peak {} then {} MB, their GPU copies {} MB",
            4 * PAGES,
            mb(one.0),
            mb(four.0),
            mb(four.1)
        );
        assert!(
            grew <= four.1 + bound,
            "{} lightmap pages peaked {} MB higher than {PAGES}, past their GPU copies' {} MB \
             + two staging budgets and a page each ({} MB) + margin: their writes were \
             held in staging until a submit (a raw `write_texture`, or the bound not fed)",
            4 * PAGES,
            mb(grew),
            mb(four.1),
            mb(bound - MARGIN)
        );
    });
}
