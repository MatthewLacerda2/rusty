use glam::Vec3;

use super::super::filter::smooth;
use super::super::raster::{dilate, fill_rest, TexelPoint};
use rusty_core::color::luminance;
use rusty_scene::lighting::settings::Denoiser;

/// Rings of texels a chart grows by before the denoiser sees it, so its edge has lit
/// neighbours, not black.
const PAD_PASSES: u32 = 4;

pub trait LightmapDenoiser: Send + Sync {
    /// `size`² row-major texels: linear HDR light, or direction vectors of length ≤ 1 when
    /// `directional`. On an `Err` the bake drops the message and filters that chart instead: an
    /// implementation logs it itself.
    fn denoise(&self, image: &mut [Vec3], size: u32, directional: bool) -> Result<(), String>;
}

#[derive(Clone, Copy)]
pub(super) struct Cleaner<'a> {
    pub denoiser: Denoiser,
    pub radius: u32,
    pub trained: Option<&'a dyn LightmapDenoiser>,
}

#[derive(Clone, Copy)]
pub(super) struct Chart<'a> {
    pub points: &'a [TexelPoint],
    pub size: u32,
    pub texel_world: f32,
}

impl Cleaner<'_> {
    pub fn clean(
        &self,
        chart: Chart,
        indirect: &[Vec3],
        dirs: Option<&[Vec3]>,
    ) -> (Vec<Vec3>, Option<Vec<Vec3>>) {
        let radius = match self.denoiser {
            Denoiser::None => 0,
            Denoiser::Oidn | Denoiser::Filter => self.radius,
        };
        let filter = |v: &[Vec3]| smooth(chart.points, v, chart.size, radius, chart.texel_world);
        if let (Denoiser::Oidn, Some(trained)) = (self.denoiser, self.trained) {
            if let Some((clean, cleaned_dirs)) = trained_clean(trained, chart, indirect, dirs) {
                return (filter(&clean), cleaned_dirs.as_deref().map(filter));
            }
        }
        (filter(indirect), dirs.map(filter))
    }
}

fn trained_clean(
    trained: &dyn LightmapDenoiser,
    chart: Chart,
    indirect: &[Vec3],
    dirs: Option<&[Vec3]>,
) -> Option<(Vec<Vec3>, Option<Vec<Vec3>>)> {
    let clean = denoised(trained, chart, indirect, false)?;
    let Some(dirs) = dirs else {
        return Some((clean, None));
    };
    let unit = |(d, l): (&Vec3, &Vec3)| match luminance(*l) {
        w if w > 0.0 => (*d / w).clamp_length_max(1.0),
        _ => Vec3::ZERO,
    };
    let unit: Vec<Vec3> = dirs.iter().zip(indirect).map(unit).collect();
    let unit = denoised(trained, chart, &unit, true)?;
    let scaled = unit.iter().zip(&clean);
    let dirs = scaled
        .map(|(d, l)| d.clamp_length_max(1.0) * luminance(*l))
        .collect();
    Some((clean, Some(dirs)))
}

fn denoised(
    trained: &dyn LightmapDenoiser,
    chart: Chart,
    values: &[Vec3],
    directional: bool,
) -> Option<Vec<Vec3>> {
    let size = chart.size;
    let mut image = vec![Vec3::ZERO; (size * size) as usize];
    let mut filled = vec![false; image.len()];
    for (p, v) in chart.points.iter().zip(values) {
        image[p.at()] = *v;
        filled[p.at()] = true;
    }
    dilate(&mut image, &mut filled, size, PAD_PASSES);
    fill_rest(&mut image, &filled, size);
    trained.denoise(&mut image, size, directional).ok()?;
    let clean = chart.points.iter().map(|p| image[p.at()]);
    let keep =
        |(c, v): (Vec3, &Vec3)| match c.is_finite() && (directional || c.min_element() >= 0.0) {
            true => c,
            false => *v,
        };
    Some(clean.zip(values).map(keep).collect())
}
