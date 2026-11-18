//! One chart's traced texels as its maps (#438, #810, #1695): the lightmap, the
//! direction map of a directional bake and the shadowmask of a `Shadowmask` one.

use glam::{Vec3, Vec4};

use super::super::raster::fill_within;
use super::super::trace::TexelLight;
use super::denoise::{Chart, Cleaner};
use super::direction::direction_texel;
use super::map::Kept;
use rusty_scene::lighting::settings::BakeSettings;

/// A chart's maps as a finished [`Lightmap`](super::Lightmap) keeps them: the
/// texels its charts cover, by row-major index, and each one's values.
pub(super) struct ChartMaps {
    pub filled: Vec<u32>,
    pub kept: Kept,
    /// How many of the chart's UV islands had no valid texel to fill the others
    /// from (#1949): their invalid texels were filled by the map's dilation.
    pub orphans: usize,
}

/// One chart's traced texels as its `chart.size`² lightmap and, for a directional bake,
/// its direction map, and with `masked` lights its shadowmask (#1695, raw: a
/// shadow is a direct-light term, never filtered): the bounce (and its directions)
/// cleaned by `cleaner` (#1884: OIDN, the Gaussian or nothing), the direct light
/// added, and an invalid texel (one `labels` gives a chart but `chart.points`
/// leaves out) filled from its own chart's valid texels (#1949,
/// `raster::fill_within`). The dilation past the chart's edge and the fill of the
/// rest of the square (for the mips, #1694) wait for the page (`map`).
pub(super) fn assemble(
    chart: Chart,
    labels: &[u32],
    light: &[TexelLight],
    settings: &BakeSettings,
    cleaner: Cleaner,
    masked: bool,
) -> ChartMaps {
    let (points, size) = (chart.points, chart.size);
    let indirect: Vec<Vec3> = light.iter().map(|l| l.indirect).collect();
    let dirs: Option<Vec<Vec3>> = settings
        .directional
        .then(|| light.iter().map(|l| l.indirect_dir).collect());
    // The bounce's directions are cleaned with its colour, so the direction page
    // is no noisier than the lightmap it reshapes.
    let (indirect, dirs) = cleaner.clean(chart, &indirect, dirs.as_deref());
    let indirect_dir = dirs.unwrap_or_else(|| vec![Vec3::ZERO; points.len()]);
    let mut texels = vec![Vec3::ZERO; (size * size) as usize];
    let mut directions = vec![Vec4::ZERO; texels.len()];
    let mut masks = vec![Vec4::ONE; texels.len()];
    let mut filled = vec![false; texels.len()];
    for (k, (point, light)) in points.iter().zip(light).enumerate() {
        texels[point.at()] = indirect[k] + light.direct;
        directions[point.at()] = direction_texel(indirect[k], indirect_dir[k], light);
        masks[point.at()] = light.mask;
        filled[point.at()] = true;
    }
    let unfilled = filled.clone();
    let orphans = fill_within(&mut texels, &mut filled, labels, size);
    // The square past the charts is filled when its page is composed
    // (`Lightmap::texels`, #2312): a finished map keeps only its charts' texels.
    let kept: Vec<u32> = (0..filled.len() as u32)
        .filter(|&i| filled[i as usize])
        .collect();
    let keep = |on: bool, mut map: Vec<Vec4>| {
        on.then(|| {
            let mut ring = unfilled.clone();
            fill_within(&mut map, &mut ring, labels, size);
            // The same charts and the same texels filled: one set of indices.
            debug_assert!(ring == filled);
            kept.iter().map(|&i| map[i as usize]).collect()
        })
    };
    let directions = keep(settings.directional, directions);
    let masks = keep(masked, masks);
    let texels = kept.iter().map(|&i| texels[i as usize]).collect();
    ChartMaps {
        filled: kept,
        kept: (texels, directions, masks),
        orphans,
    }
}
