use std::fmt;

use glam::Vec2;
use rusty_lighting::lightmap::{lightmap_size, max_map_edge, BakeMesh, BakeScene, ASSEMBLE_ALONE};
use rusty_scene::lighting::settings::{page_edge, BakeSettings, Denoiser};

use crate::bake::denoise::budget;
use crate::bake::progress::format::gb;
use crate::memory::Machine;

// Bytes per texel each stage holds: in flight and progressive per covered texel, the other
// three per texel of their square.
pub const IN_FLIGHT_PER_TEXEL: u64 = 112;

pub const FINISH_PER_TEXEL: u64 = 256;

pub const PAGE_PER_TEXEL: u64 = 128;

pub const EXPAND_PER_TEXEL: u64 = 64;

pub const PROGRESSIVE_PER_TEXEL: u64 = 116;

/// Bytes a finished map keeps per covered texel; mirrors `Lightmap::kept_bytes`.
pub fn result_bytes(settings: &BakeSettings, masked: bool) -> u64 {
    16 + 16 * u64::from(settings.directional) + 16 * u64::from(masked)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Estimate {
    pub maps: u64,
    pub in_flight: u64,
    pub oidn: u64,
    pub pages: u64,
    pub progressive: u64,
}

impl Estimate {
    pub fn peak(&self) -> u64 {
        // A chart is denoised while it is assembled: the larger stands for both, as measured.
        // The pages come after that work, so the peak holds the larger again.
        let work = self.in_flight.max(self.oidn);
        self.maps + self.progressive + work.max(self.pages)
    }
}

impl fmt::Display for Estimate {
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
    // The UVs' area plus a texel-wide ring along every edge (the conservative raster).
    let e = f64::from(edge);
    let square = u64::from(edge) * u64::from(edge);
    ((area * e * e + perimeter * e).ceil() as u64).min(square)
}

pub fn estimate(
    input: &BakeScene,
    settings: &BakeSettings,
    progressive: bool,
    threads: usize,
) -> Estimate {
    let max = max_map_edge(settings.page());
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
    // Past `ASSEMBLE_ALONE` one chart assembles at a time, so `largest` stands for it.
    let beside = |&(square, covered): &(u64, u64)| match square > ASSEMBLE_ALONE {
        true => covered * IN_FLIGHT_PER_TEXEL,
        false => (covered * IN_FLIGHT_PER_TEXEL).max(square * FINISH_PER_TEXEL),
    };
    let in_flight = largest * FINISH_PER_TEXEL + flying.iter().skip(1).map(beside).sum::<u64>();
    let oidn = match settings.denoiser {
        Denoiser::Oidn => {
            let denoise: u64 = flying.iter().map(|c| budget::cost(c.0 as usize)).sum();
            // Mirrors `budget::Budget::run`: past `BUDGET` one chart runs alone, and always starts.
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

pub fn fits(input: &BakeScene, settings: &BakeSettings, progressive: bool) -> Result<(), String> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let estimate = estimate(input, settings, progressive, threads);
    check(estimate, settings, crate::memory::machine())
}
