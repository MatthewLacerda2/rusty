//! A scene's textures loaded from their import caches hold each one's blocks only
//! until its upload (#2372): staging is bounded (`rusty_render::STAGING_BUDGET`),
//! so four times the textures peak higher only by their GPU copies, and a cached
//! load peaks no higher than the same textures decoded from their PNGs
//! (`decoded_gpu_tests`). Each test runs in a process of its own
//! (`rusty_test_support::alone!`), so the peak (`memory::rss_peak_bytes`) is its
//! loads'.
//!
//! The GPU copies count on both sides: lavapipe's and Apple silicon's textures sit
//! in the memory the process's footprint counts. They are read from the device
//! (`rusty_render::gpu_memory`, 0 where a backend reports none) and taken off the
//! growth, so what is asserted is what the load held beside them.

use std::path::Path;

use rusty_asset::sidecar::{set_texture, Compression, TextureSettings};
use rusty_asset::texture_import::write_blank;
use rusty_asset::MapRef;
use rusty_render::{gpu_memory, Renderer, STAGING_BUDGET};

use crate::memory::rss_peak_bytes;

/// The textures' side: a level's map.
const SIDE: u32 = 2048;
/// The smaller load; the larger is four times it.
pub(super) const N: usize = 8;
/// Slack on top of the bound: the allocator's and the driver's.
const MARGIN: u64 = 24 << 20;

/// `n` copies of one flat `SIDE`² image in `dir`, named `name<i>.png`: with an
/// import cache each when `cached` (blank blocks, `texture_import::write_blank`,
/// so no encode lifts the peak before the loads), else with compression off.
pub(super) fn images(dir: &Path, name: &str, n: usize, cached: bool) -> Vec<String> {
    let source = dir.join("source.png");
    if !source.exists() {
        let image = image::RgbaImage::from_pixel(SIDE, SIDE, image::Rgba([90, 120, 200, 255]));
        image.save(&source).expect("saved");
    }
    let settings = TextureSettings {
        compression: match cached {
            true => Compression::default(),
            false => Compression::None,
        },
        ..Default::default()
    };
    (0..n)
        .map(|i| {
            let path = dir.join(format!("{name}{i}.png"));
            std::fs::copy(&source, &path).expect("copied");
            set_texture(&path, settings).expect("set");
            if cached {
                write_blank(&path, &settings).expect("cached");
            }
            path.to_string_lossy().into_owned()
        })
        .collect()
}

/// What the device's live allocations hold now, after every queued upload ran.
fn device_bytes(renderer: &Renderer) -> u64 {
    renderer.queue.submit([]);
    let memory = gpu_memory(&renderer.device);
    // Metal's figure is allocated bytes already.
    memory.map_or(0, |m| m.allocated.unwrap_or(m.bytes))
}

/// Load `paths` as a frame's lookups do: the peak after, and what the device
/// holds for them.
pub(super) fn load(renderer: &mut Renderer, paths: &[String]) -> (u64, u64) {
    let before = device_bytes(renderer);
    for path in paths {
        renderer.load_texture(&MapRef::from(path.as_str()));
    }
    let held = device_bytes(renderer) - before;
    (rss_peak_bytes().expect("read"), held)
}

/// A renderer and a fresh folder, or `None` where no peak or adapter is had.
pub(super) fn set_up(name: &str) -> Option<(Renderer, std::path::PathBuf)> {
    rss_peak_bytes()?;
    crate::memory::return_large_frees();
    let renderer = rusty_render::test_gpu::headless_or_skip(16, 16)?;
    let dir = rusty_test_support::temp::dir().join(format!("{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("made");
    Some((renderer, dir))
}

pub(super) fn mb(bytes: u64) -> u64 {
    bytes >> 20
}

/// Whether the first of `cached` uploads compressed: with no BC on the adapter, no
/// cache is read, and there is nothing to measure.
pub(super) fn compresses(renderer: &mut Renderer, cached: &[String]) -> bool {
    let first = renderer.load_texture(&MapRef::from(cached[0].as_str()));
    first.texture.format().is_compressed()
}

#[test]
fn gpu_four_times_the_cached_textures_peak_higher_only_by_their_gpu_copies() {
    rusty_test_support::alone!(|| {
        let Some((mut renderer, dir)) = set_up("staging_bound") else {
            return;
        };
        let (few, many) = (
            images(&dir, "few", N, true),
            images(&dir, "many", 4 * N, true),
        );
        if !compresses(&mut renderer, &few) {
            return;
        }
        let one = load(&mut renderer, &few[1..]);
        let four = load(&mut renderer, &many);
        let grew = four.0.saturating_sub(one.0);
        // In staging at once: the uploads pending and those of the submit before,
        // each at most the budget plus the one upload that crossed it.
        let bound = 2 * (STAGING_BUDGET + four.1 / (4 * N as u64)) + MARGIN;
        println!(
            "{N} then {}: peak {} then {} MB",
            4 * N,
            mb(one.0),
            mb(four.0)
        );
        assert!(
            grew <= four.1 + bound,
            "{} cached textures peaked {} MB higher than {N}, past their GPU copies' {} MB \
             + two staging budgets and a texture each ({} MB) + margin: their blocks were \
             held past their upload (staging never submitted, or a CPU copy kept)",
            4 * N,
            mb(grew),
            mb(four.1),
            mb(bound - MARGIN)
        );
    });
}
