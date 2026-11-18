//! Whether a lightmap bake fits the machine, before it starts (#2312): an
//! estimate of the memory it will add at its peak, from its meshes' lightmap
//! edges and its settings, held to the memory the machine has. Past what is free
//! the bake warns, naming the settings that bring it down; past all the machine
//! has it is refused, since it would only run into the out-of-memory killer.
//!
//! What a one-shot bake holds (measured in #2312's PR): every finished lightmap
//! ([`result_bytes`]), the charts in flight (a worker's chart: its texels and
//! their traced light, [`IN_FLIGHT_PER_TEXEL`]; the one being assembled,
//! [`FINISH_PER_TEXEL`]), and OIDN's charts in flight, at most
//! `denoise::budget::BUDGET`. Once traced, the maps are packed into pages: about
//! the results again. A progressive bake (the editor's) also keeps every texel's
//! state between passes ([`PROGRESSIVE_PER_TEXEL`]). Each texel is counted at its
//! map's full edge², so the estimate errs high.

use rusty_lighting::lightmap::{lightmap_size, max_map_edge, BakeScene};
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

use crate::bake::denoise::budget;
use crate::bake::progress::format::gb;
use crate::memory::Machine;

/// A chart in flight on a worker: its points (32 bytes) and traced light (80).
pub const IN_FLIGHT_PER_TEXEL: u64 = 112;

/// The chart being assembled into its maps: its points and light, their
/// filtered copies, the denoise's images and the maps themselves.
pub const FINISH_PER_TEXEL: u64 = 256;

/// A progressive bake's texel between passes (`Texel`) and its chart label.
pub const PROGRESSIVE_PER_TEXEL: u64 = 116;

/// What one finished lightmap texel keeps: its light (12 bytes), its direction
/// (16) in a directional bake, its shadowmask (16) in a masked one.
pub fn result_bytes(settings: &BakeSettings, masked: bool) -> u64 {
    12 + 16 * u64::from(settings.directional) + 16 * u64::from(masked)
}

/// The bytes a bake of `input` at `settings` adds at its peak on `threads`
/// workers, `progressive` for the editor's bake.
pub fn estimate(
    input: &BakeScene,
    settings: &BakeSettings,
    progressive: bool,
    threads: usize,
) -> u64 {
    let max = max_map_edge(settings.page());
    let mut charts: Vec<u64> = (input.meshes.iter())
        .filter(|m| m.receives_lightmap())
        .filter_map(|m| lightmap_size(m, settings.texels_per_unit * m.scale_in_lightmap, max))
        .map(|edge| u64::from(edge) * u64::from(edge))
        .collect();
    charts.sort_unstable_by(|a, b| b.cmp(a));
    let all: u64 = charts.iter().sum();
    let results = all * result_bytes(settings, input.shadowmask.any());
    let flying = &charts[..threads.max(1).min(charts.len())];
    let largest = flying.first().copied().unwrap_or(0);
    let mut work = largest * FINISH_PER_TEXEL;
    work += flying.iter().skip(1).sum::<u64>() * IN_FLIGHT_PER_TEXEL;
    if settings.denoiser == Denoiser::Oidn {
        let denoise: u64 = flying.iter().map(|&t| budget::cost(t as usize)).sum();
        work += denoise.min(budget::BUDGET.max(budget::cost(largest as usize)));
    }
    let kept = if progressive {
        all * PROGRESSIVE_PER_TEXEL
    } else {
        0
    };
    kept + results + work.max(results)
}

/// Hold a bake estimated to add `peak` bytes to `machine` (`None` when it cannot
/// be read, which lets every bake start): `Err` when it passes all the machine
/// has, a warning when it passes what is free.
pub fn check(peak: u64, settings: &BakeSettings, machine: Option<Machine>) -> Result<(), String> {
    let Some(machine) = machine else {
        return Ok(());
    };
    let say = |what: &str, has: u64| {
        format!(
            "this lightmap bake needs about {} at its peak, more than the {} {what}: \
             lower texelsPerUnit ({}) or maxLightmapSize ({}){}",
            gb(peak),
            gb(has),
            settings.texels_per_unit,
            settings.max_lightmap_size,
            match settings.directional {
                true => ", or turn off directional",
                false => "",
            }
        )
    };
    if peak > machine.total {
        return Err(say("this machine has in all", machine.total));
    }
    if peak > machine.available {
        log::warn!("[LightmapBake] {}", say("it has free", machine.available));
    }
    Ok(())
}

/// Hold a bake of `input` at `settings` to this machine ([`estimate`], [`check`]).
pub fn fits(input: &BakeScene, settings: &BakeSettings, progressive: bool) -> Result<(), String> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let peak = estimate(input, settings, progressive, threads);
    check(peak, settings, crate::memory::machine())
}
