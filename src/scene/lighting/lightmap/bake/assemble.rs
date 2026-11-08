//! One chart's traced texels as its maps (#438, #810, #1695): the lightmap, the
//! direction map of a directional bake and the shadowmask of a `Shadowmask` one.

use glam::{Vec3, Vec4};

use super::super::raster::{dilate, fill_rest, TexelPoint};
use super::super::trace::TexelLight;
use super::denoise::{Chart, Cleaner};
use super::direction::direction_texel;
use super::BakeSettings;

/// Rings of empty texels dilation fills around each chart.
const DILATE_PASSES: u32 = 3;

/// A chart's maps, each `size`² texels or empty when the bake makes none.
pub(super) struct ChartMaps {
    pub texels: Vec<Vec3>,
    pub directions: Vec<Vec4>,
    pub masks: Vec<Vec4>,
}

/// One chart's traced texels as its `size`² lightmap and, for a directional bake,
/// its direction map, and with `masked` lights its shadowmask (#1695, raw: a
/// shadow is a direct-light term, never filtered): the bounce (and its directions)
/// cleaned by `cleaner` (#1884: OIDN, the Gaussian or nothing), the direct light
/// added, then each dilated past the chart's edge and the rest of the map filled
/// from its nearest chart (`raster::fill_rest`, for the mips, #1694).
pub(super) fn assemble(
    points: &[TexelPoint],
    light: &[TexelLight],
    size: u32,
    texel_world: f32,
    settings: &BakeSettings,
    cleaner: Cleaner,
    masked: bool,
) -> ChartMaps {
    let indirect: Vec<Vec3> = light.iter().map(|l| l.indirect).collect();
    let dirs: Option<Vec<Vec3>> = settings
        .directional
        .then(|| light.iter().map(|l| l.indirect_dir).collect());
    // The bounce's directions are cleaned with its colour, so the direction page
    // is no noisier than the lightmap it reshapes.
    let chart = Chart {
        points,
        size,
        texel_world,
    };
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
    for (keep, map) in [
        (settings.directional, &mut directions),
        (masked, &mut masks),
    ] {
        if keep {
            let mut ring = filled.clone();
            dilate(map, &mut ring, size, DILATE_PASSES);
            fill_rest(map, &ring, size);
        } else {
            map.clear();
        }
    }
    dilate(&mut texels, &mut filled, size, DILATE_PASSES);
    fill_rest(&mut texels, &filled, size);
    ChartMaps {
        texels,
        directions,
        masks,
    }
}
