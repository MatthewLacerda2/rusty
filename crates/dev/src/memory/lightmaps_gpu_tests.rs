use rusty_render::{Renderer, STAGING_BUDGET};

use super::textures_gpu_tests::{device_bytes, mb, set_up};
use crate::memory::rss_peak_bytes;

/// Texels on a page's edge: one `Rgba16Float` level is 8 MiB.
const EDGE: u32 = 1024;
/// Pages in the smaller list; the larger is four times it.
const PAGES: usize = 4;
/// Bytes of slack on the bound: the allocator's and the driver's.
const MARGIN: u64 = 24 << 20;

/// A nonzero byte touches every page, so each is resident before the measure: zeroed ones
/// come lazily mapped from calloc on Linux.
fn pages(n: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|_| vec![0x3c; (EDGE * EDGE * 8) as usize])
        .collect()
}

/// Also returns the texture and pages, kept live: where GPU copies are host memory
/// (lavapipe), a page freed before its copy ran would make room for it and hide staging.
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
