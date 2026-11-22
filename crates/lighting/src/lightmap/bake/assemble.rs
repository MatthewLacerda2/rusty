use glam::{Vec3, Vec4};

use super::super::raster::fill_within;
use super::super::trace::TexelLight;
use super::denoise::{Chart, Cleaner};
use super::direction::direction_texel;
use super::map::Kept;
use rusty_scene::lighting::settings::BakeSettings;

pub(super) struct ChartMaps {
    pub filled: Vec<u32>,
    pub kept: Kept,
    pub orphans: usize,
}

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
    let mut kept = Vec::with_capacity(filled.iter().filter(|&&f| f).count());
    kept.extend((0..filled.len() as u32).filter(|&i| filled[i as usize]));
    let keep = |on: bool, mut map: Vec<Vec4>| {
        on.then(|| {
            let mut ring = unfilled.clone();
            fill_within(&mut map, &mut ring, labels, size);
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
