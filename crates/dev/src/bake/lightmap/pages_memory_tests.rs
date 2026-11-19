//! A bake's memory through its page stage (#2312), on the shape that was killed:
//! many meshes whose lightmap squares their UVs cover only a sliver of, a page
//! each. Four times the meshes may peak higher only by what their finished maps
//! keep of their covered texels, through the trace and the pages: a map that
//! kept its whole square, or a page stage that held every page it made, grows
//! with the squares instead. Each test runs in a process of its own
//! (`rusty_test_support::alone!`), so the peak
//! (`memory::rss_peak_bytes`) is this test's bakes'.

use std::sync::Arc;

use glam::Vec2;

use super::memory_tests::{kept, planes, MARGIN, WORKERS};
use super::tests::scene_path;
use super::*;
use crate::bake::progress::MeshClocks;
use crate::memory::rss_peak_bytes;
use rusty_lighting::lightmap::{bake_on, BakeScene};
use rusty_scene::lighting::settings::Denoiser;

/// The share of its UV square's height each sliver's UVs cover.
const BAND: f32 = 0.1;

/// `n` 20-unit planes whose UVs cover a band [`BAND`] of their square tall: at
/// 16 texels a unit, each a 1012² square, about a tenth of it covered.
fn slivers(n: usize) -> BakeScene {
    let mut scene = planes(n, 20.0);
    for mesh in &mut scene.meshes {
        let (low, high) = (Vec2::ZERO, Vec2::new(1.0, BAND));
        mesh.lightmap_uvs = vec![low, Vec2::new(1.0, 0.0), high, Vec2::new(0.0, BAND)];
    }
    scene
}

/// The cheapest trace, directional, on 1024² pages: a sliver's square fills one.
fn settings() -> BakeSettings {
    BakeSettings {
        texels_per_unit: 16.0,
        samples: 1,
        bounces: 0,
        denoiser: Denoiser::Filter,
        filter_radius: 1,
        directional: true,
        max_lightmap_size: 1024,
        ..Default::default()
    }
}

/// Bake and encode `n` slivers, spilled for a scene of its own: the process's
/// peak after, what the maps kept, and the pages made.
fn baked(n: usize) -> (u64, u64, usize) {
    crate::memory::return_large_frees();
    let progress = BakeProgress::observed(Arc::new(MeshClocks::default()));
    let s = settings();
    let maps = bake_on(&slivers(n), &s, &progress, WORKERS).expect("ran");
    let square: usize = maps.iter().map(|m| (m.size * m.size) as usize).sum();
    assert!(
        maps.iter()
            .all(|m| m.covered() * 5 < (m.size * m.size) as usize),
        "each map covers a sliver of its square"
    );
    let kept = kept(&maps);
    let path = scene_path(&format!("pages_memory_{n}"));
    let pages = encode_pages(maps, s.page(), &progress, &path)
        .expect("encoded")
        .expect("never cancelled");
    let peak = rss_peak_bytes().expect("read on Linux and macOS");
    println!(
        "{n} slivers: peak {} MB, maps keep {} MB of {} MB squares, {} pages, {:?}",
        peak >> 20,
        kept >> 20,
        (square * 28) >> 20,
        pages.colour.len() + pages.directions.len(),
        crate::memory::heap()
    );
    (peak, kept, pages.colour.len())
}

#[test]
fn four_times_the_sparse_meshes_peak_higher_only_by_their_covered_texels() {
    if rss_peak_bytes().is_none() {
        return;
    }
    rusty_test_support::alone!(|| {
        let one = baked(2);
        let four = baked(8);
        assert_eq!((one.2, four.2), (2, 8), "a page apiece");
        let (grew, extra) = (four.0.saturating_sub(one.0), four.1 - one.1);
        let mb = |b: u64| b >> 20;
        assert!(
            grew <= extra + MARGIN,
            "four times the sliver meshes peaked {} MB higher ({} MB against {} MB) through \
             the trace and the pages, but their extra maps keep only {} MB of covered texels: \
             a square or a page is held per mesh of the scene",
            mb(grew),
            mb(four.0),
            mb(one.0),
            mb(extra)
        );
    });
}
