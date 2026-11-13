//! Cleaning a chart's baked bounce (#1884): which denoiser a bake runs, and the
//! seam a trained one plugs into.
//!
//! Unity's Progressive Lightmapper picks a *Denoiser* (OIDN, OptiX, None) and a
//! *Filter* (Gaussian, A-Trous, None), and runs the denoiser first, then the
//! filter. rusty folds both into one choice, [`Denoiser`]: Intel Open Image
//! Denoise's lightmap model followed by our edge-aware Gaussian (`Oidn`), the
//! Gaussian alone (`Filter`, `filter::smooth`), or the raw trace (`None`). The pair
//! gets from 64 paths per texel what the Gaussian alone needed 128 for. OIDN alone
//! at 32 or 64 leaves a few-texel mottling on flat bounce-lit walls, which the
//! Gaussian after it removes (measured and looked at in #1884's PR).
//!
//! OIDN is a native library and authoring-only, so the sim never names it: the
//! platform lends the bake a [`LightmapDenoiser`] on its `BakeProgress`
//! (`BakeProgress::with_denoiser`), as it lends an observer (#1891). A bake with no
//! denoiser lent, or whose denoiser cannot run, filters instead, so an `Oidn` bake
//! always finishes. Only the bounce is cleaned: the direct light stays sharp, as
//! under the filter.
//!
//! Determinism: the trace is untouched, so a `Filter` or `None` bake keeps its
//! bytes on every machine. A trained denoiser's output may differ between CPUs
//! (its SIMD path), never between runs or thread counts on one machine; the sim
//! reads only the stored pages, so the determinism rule holds either way.

use glam::Vec3;

use super::super::filter::smooth;
use super::super::raster::{dilate, fill_rest, TexelPoint};
use rusty_core::color::luminance;

/// Rings of texels the chart is grown by before a trained denoiser sees it, so the
/// texels at its edge have lit neighbours instead of black ones.
const PAD_PASSES: u32 = 4;

/// Which denoiser cleans a lightmap's bounce light (Unity's Lighting settings'
/// *Denoiser*). A scene file without one takes [`Denoiser::Oidn`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Denoiser {
    /// Intel Open Image Denoise's `RTLightmap` filter (HDR, trained on lightmaps),
    /// run by the platform, then the Gaussian as under [`Denoiser::Filter`]; where
    /// OIDN is not installed the bake warns and only filters.
    #[default]
    Oidn,
    /// The edge-aware Gaussian, [`BakeSettings::filter_radius`](super::BakeSettings::filter_radius) texels wide.
    Filter,
    /// Neither: the bounce stays as traced.
    None,
}

impl Denoiser {
    /// Every denoiser, in the order the editor lists them.
    pub const ALL: [Denoiser; 3] = [Denoiser::Oidn, Denoiser::Filter, Denoiser::None];

    /// The name the editor and `Lighting.GetSettings()` use.
    pub fn name(self) -> &'static str {
        match self {
            Denoiser::Oidn => "OIDN",
            Denoiser::Filter => "Filter",
            Denoiser::None => "None",
        }
    }

    /// The denoiser `name` names (any case), as [`Self::name`] spells them.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|d| d.name().eq_ignore_ascii_case(name))
    }
}

/// Runs a trained denoiser over one chart (#1884). The platform implements it
/// (`dev::bake::denoise`); the sim only calls it. Charts are cleaned on the bake's
/// workers, so calls may come from several threads at once.
pub trait LightmapDenoiser: Send + Sync {
    /// Clean `image` in place: `size`² texels, row-major, linear HDR light, or for
    /// a `directional` image direction vectors of length ≤ 1. `Err` says why it
    /// could not run, and the bake filters that chart instead.
    fn denoise(&self, image: &mut [Vec3], size: u32, directional: bool) -> Result<(), String>;
}

/// The cleaner one chart's bounce goes through: `settings`' choice, with what the
/// platform lent for `Oidn`.
#[derive(Clone, Copy)]
pub(super) struct Cleaner<'a> {
    pub denoiser: Denoiser,
    pub radius: u32,
    pub trained: Option<&'a dyn LightmapDenoiser>,
}

/// One chart as the cleaner sees it: its texels and their size in the world.
#[derive(Clone, Copy)]
pub(super) struct Chart<'a> {
    pub points: &'a [TexelPoint],
    pub size: u32,
    pub texel_world: f32,
}

impl Cleaner<'_> {
    /// The chart's bounce `indirect` (one per point) cleaned, and with `dirs` its
    /// luminance-weighted directions (`TexelLight::indirect_dir`) cleaned to match.
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

/// [`Cleaner::clean`] through `trained`: the bounce as HDR colour, and its
/// directions as unit-bounded vectors (each divided by its texel's luminance, then
/// scaled by the cleaned one). `None` when the denoiser failed.
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

/// `values` through `trained` on the chart's whole image, its edge grown so the
/// denoiser sees no black border, read back at the points. `None` when it failed.
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
    // The platform says why once (`dev::bake::denoise`); here the chart just filters.
    trained.denoise(&mut image, size, directional).ok()?;
    let clean = chart.points.iter().map(|p| image[p.at()]);
    // A NaN or a negative from the denoiser would poison the page; keep the trace.
    let keep =
        |(c, v): (Vec3, &Vec3)| match c.is_finite() && (directional || c.min_element() >= 0.0) {
            true => c,
            false => *v,
        };
    Some(clean.zip(values).map(keep).collect())
}
