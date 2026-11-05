//! One chart's traced texels as its maps (#438, #810, #1695): the lightmap, the
//! direction map of a directional bake and the shadowmask of a `Shadowmask` one.

use glam::{Vec3, Vec4};

use super::super::filter::smooth;
use super::super::raster::{dilate, fill_rest, TexelPoint};
use super::super::trace::TexelLight;
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
/// shadow is a direct-light term, never filtered): the bounce (and its directions) smoothed, the direct light
/// added, then each dilated past the chart's edge and the rest of the map filled
/// from its nearest chart (`raster::fill_rest`, for the mips, #1694).
pub(super) fn assemble(
    points: &[TexelPoint],
    light: &[TexelLight],
    size: u32,
    texel_world: f32,
    settings: &BakeSettings,
    masked: bool,
) -> ChartMaps {
    let indirect: Vec<Vec3> = light.iter().map(|l| l.indirect).collect();
    let radius = settings.filter_radius;
    let indirect = smooth(points, &indirect, size, radius, texel_world);
    // The bounce's directions are smoothed by the same kernel as its colour, so
    // the direction page is no noisier than the lightmap it reshapes.
    let indirect_dir: Vec<Vec3> = match settings.directional {
        true => {
            let dirs: Vec<Vec3> = light.iter().map(|l| l.indirect_dir).collect();
            smooth(points, &dirs, size, radius, texel_world)
        }
        false => vec![Vec3::ZERO; points.len()],
    };
    let mut texels = vec![Vec3::ZERO; (size * size) as usize];
    let mut directions = vec![Vec4::ZERO; texels.len()];
    let mut masks = vec![Vec4::ONE; texels.len()];
    let mut filled = vec![false; texels.len()];
    for (k, (point, light)) in points.iter().zip(light).enumerate() {
        texels[point.index] = indirect[k] + light.direct;
        directions[point.index] = direction_texel(indirect[k], indirect_dir[k], light);
        masks[point.index] = light.mask;
        filled[point.index] = true;
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
