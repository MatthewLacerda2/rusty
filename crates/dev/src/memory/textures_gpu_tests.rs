//! These tests bite only where GPU copies sit in resident memory (lavapipe, Apple silicon):
//! on a discrete GPU they cannot fail.

use std::path::Path;

use rusty_asset::sidecar::{set_texture, Compression, TextureSettings};
use rusty_asset::texture_import::write_blank;
use rusty_asset::MapRef;
use rusty_render::{gpu_memory, Renderer, STAGING_BUDGET};

use crate::memory::rss_peak_bytes;

/// Texels on each image's side, a level map's size.
const SIDE: u32 = 2048;
/// Textures in the smaller load; the larger is four times it.
pub(super) const N: usize = 8;
/// Bytes of slack on the bound: the allocator's and the driver's.
const MARGIN: u64 = 24 << 20;

/// A cached image gets blank blocks (`write_blank`): a real encode would lift the peak before
/// the loads are measured.
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

/// The device's live allocations, read after an empty submit sends the queued uploads,
/// which `gpu_memory`'s poll waits for.
pub(super) fn device_bytes(renderer: &Renderer) -> u64 {
    renderer.queue.submit([]);
    let memory = gpu_memory(&renderer.device);
    // Allocated, not reserved, bytes: Metal's `bytes` is allocated already.
    memory.map_or(0, |m| m.allocated.unwrap_or(m.bytes))
}

pub(super) fn load(renderer: &mut Renderer, paths: &[String]) -> (u64, u64) {
    let before = device_bytes(renderer);
    for path in paths {
        renderer.load_texture(&MapRef::from(path.as_str()));
    }
    let held = device_bytes(renderer) - before;
    (rss_peak_bytes().expect("read"), held)
}

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
        let bound = 2 * (STAGING_BUDGET + four.1 / (4 * N as u64)) + MARGIN;
        println!(
            "{N} then {}: peak {} then {} MB, their GPU copies {} MB",
            4 * N,
            mb(one.0),
            mb(four.0),
            mb(four.1)
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
