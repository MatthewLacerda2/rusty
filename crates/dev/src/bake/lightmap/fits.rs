//! Whether a lightmap bake fits the machine, before it starts (#2312): an
//! estimate of the memory it will add at its peak, from its meshes' lightmap
//! edges and its settings, held to the memory the machine has. Past what is free
//! the bake warns, naming the settings that bring it down; past all the machine
//! has it is refused, since it would only run into the out-of-memory killer.
//!
//! What a one-shot bake holds (measured in #2312's PR): every finished lightmap,
//! only the texels its charts cover ([`result_bytes`] each, `lightmap::Lightmap`);
//! the charts in flight (a worker's chart: its covered texels and their traced
//! light, [`IN_FLIGHT_PER_TEXEL`]; the one being assembled, over its whole square,
//! [`FINISH_PER_TEXEL`]), or OIDN's charts in flight (at most
//! `denoise::budget::BUDGET`) when those are more, since a chart is denoised while
//! it is assembled. Once traced, the pages are composed, encoded and spilled one
//! at a time (`pages::encode_pages`): one page in flight ([`PAGE_PER_TEXEL`] of
//! its square) and the map being filled into it ([`EXPAND_PER_TEXEL`] of the
//! largest map's square). A progressive bake (the editor's) also keeps every
//! texel's state between passes ([`PROGRESSIVE_PER_TEXEL`]). A mesh's covered
//! texels are read from its lightmap UVs' area, with a texel's ring around each
//! triangle for the conservative raster, so the estimate errs high.

use std::fmt;

use glam::Vec2;
use rusty_lighting::lightmap::{lightmap_size, max_map_edge, BakeMesh, BakeScene};
use rusty_scene::lighting::settings::{page_edge, BakeSettings, Denoiser};

use crate::bake::denoise::budget;
use crate::bake::progress::format::gb;
use crate::memory::Machine;

/// A chart in flight on a worker, per covered texel: its point (32 bytes) and
/// traced light (80).
pub const IN_FLIGHT_PER_TEXEL: u64 = 112;

/// The chart being assembled into its maps, per texel of its square: its dense
/// maps and their fill state, its points and light, their filtered copies and
/// the denoise's images.
pub const FINISH_PER_TEXEL: u64 = 256;

/// The page being composed and encoded, per texel of its square: its colour,
/// direction and mask texels, their mip chains, and the encoded files before
/// they are spilled.
pub const PAGE_PER_TEXEL: u64 = 128;

/// The map being filled into its page, per texel of its square: its dense map,
/// the dilation's two copies and the fill's state.
pub const EXPAND_PER_TEXEL: u64 = 64;

/// A progressive bake's texel between passes (`Texel`) and its chart label.
pub const PROGRESSIVE_PER_TEXEL: u64 = 116;

/// What one finished lightmap keeps per covered texel: its index (4 bytes), its
/// light (12), its direction (16) in a directional bake, its shadowmask (16) in a
/// masked one.
pub fn result_bytes(settings: &BakeSettings, masked: bool) -> u64 {
    16 + 16 * u64::from(settings.directional) + 16 * u64::from(masked)
}

/// A bake's estimated peak, by what holds it. The work in flight and the pages
/// come one after the other, so the peak holds the larger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Estimate {
    /// The finished lightmaps, at the end of the trace.
    pub maps: u64,
    /// The charts the workers trace and assemble.
    pub in_flight: u64,
    /// OIDN's charts in flight, which overlap the assembly: the larger counts.
    pub oidn: u64,
    /// The page being composed and encoded, and the map filled into it.
    pub pages: u64,
    /// A progressive bake's per-texel state.
    pub progressive: u64,
}

impl Estimate {
    /// The peak: what is kept, and the larger of the trace's and the pages' work.
    pub fn peak(&self) -> u64 {
        let work = self.in_flight.max(self.oidn);
        self.maps + self.progressive + work.max(self.pages)
    }
}

impl fmt::Display for Estimate {
    /// `finished maps 1.2 GB, in flight 0.4 GB, OIDN 2.0 GB, pages 0.6 GB`, OIDN
    /// only when it runs, the progressive state only in a progressive bake.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (maps, flying) = (gb(self.maps), gb(self.in_flight));
        write!(f, "finished maps {maps}, in flight {flying}")?;
        if self.oidn > 0 {
            write!(f, ", OIDN {}", gb(self.oidn))?;
        }
        write!(f, ", pages {}", gb(self.pages))?;
        if self.progressive > 0 {
            write!(f, ", progressive state {}", gb(self.progressive))?;
        }
        Ok(())
    }
}

/// The texels of a `edge`² lightmap `mesh`'s UVs cover: their area, a texel's
/// ring around each triangle's edges (the conservative raster), at most the square.
pub fn covered(mesh: &BakeMesh, edge: u32) -> u64 {
    let uv = |i: u32| {
        mesh.lightmap_uvs
            .get(i as usize)
            .copied()
            .unwrap_or(Vec2::ZERO)
    };
    let (mut area, mut perimeter) = (0.0f64, 0.0f64);
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [uv(t[0]), uv(t[1]), uv(t[2])];
        area += f64::from((b - a).perp_dot(c - a).abs()) / 2.0;
        perimeter += f64::from(a.distance(b) + b.distance(c) + c.distance(a));
    }
    let e = f64::from(edge);
    let square = u64::from(edge) * u64::from(edge);
    ((area * e * e + perimeter * e).ceil() as u64).min(square)
}

/// What a bake of `input` at `settings` adds at its peak on `threads` workers,
/// `progressive` for the editor's bake.
pub fn estimate(
    input: &BakeScene,
    settings: &BakeSettings,
    progressive: bool,
    threads: usize,
) -> Estimate {
    let max = max_map_edge(settings.page());
    // Each map's square and covered texels, largest square first.
    let mut charts: Vec<(u64, u64)> = (input.meshes.iter())
        .filter(|m| m.receives_lightmap())
        .filter_map(|m| {
            let edge = lightmap_size(m, settings.texels_per_unit * m.scale_in_lightmap, max)?;
            Some((u64::from(edge) * u64::from(edge), covered(m, edge)))
        })
        .collect();
    if charts.is_empty() {
        return Estimate::default();
    }
    charts.sort_unstable_by(|a, b| b.cmp(a));
    let all: u64 = charts.iter().map(|c| c.1).sum();
    let flying = &charts[..threads.max(1).min(charts.len())];
    let largest = flying[0].0;
    let in_flight = largest * FINISH_PER_TEXEL
        + flying.iter().skip(1).map(|c| c.1).sum::<u64>() * IN_FLIGHT_PER_TEXEL;
    // A chart is denoised while it is assembled, so the two overlap: the larger
    // stands for both (measured in #2312's PR).
    let oidn = match settings.denoiser {
        Denoiser::Oidn => {
            let denoise: u64 = flying.iter().map(|c| budget::cost(c.0 as usize)).sum();
            denoise.min(budget::BUDGET.max(budget::cost(largest as usize)))
        }
        _ => 0,
    };
    let page = u64::from(page_edge(settings.page())).pow(2);
    Estimate {
        maps: all * result_bytes(settings, input.shadowmask.any()),
        in_flight,
        oidn,
        pages: page * PAGE_PER_TEXEL + largest * EXPAND_PER_TEXEL,
        progressive: match progressive {
            true => all * PROGRESSIVE_PER_TEXEL,
            false => 0,
        },
    }
}

/// Hold a bake estimated as `estimate` to `machine` (`None` when it cannot be
/// read, which lets every bake start): `Err` when its peak passes all the machine
/// has, a warning when it passes what is free, each with its breakdown.
pub fn check(
    estimate: Estimate,
    settings: &BakeSettings,
    machine: Option<Machine>,
) -> Result<(), String> {
    let Some(machine) = machine else {
        return Ok(());
    };
    let peak = estimate.peak();
    let say = |what: &str, has: u64| {
        format!(
            "this lightmap bake needs about {} at its peak ({estimate}), more than the {} \
             {what}: lower texelsPerUnit ({}) or maxLightmapSize ({}){}",
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

/// Hold a bake of `input` at `settings` to this machine ([`estimate`], [`check`]),
/// and have the allocator give back what the bake frees
/// (`memory::return_large_frees`), which the estimate counts on.
pub fn fits(input: &BakeScene, settings: &BakeSettings, progressive: bool) -> Result<(), String> {
    crate::memory::return_large_frees();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let estimate = estimate(input, settings, progressive, threads);
    check(estimate, settings, crate::memory::machine())
}
