//! The lightmap bake's knobs ([`BakeSettings`]), Unity's Lightmapping settings in
//! miniature, and the one place their ranges live: the denoiser a bake runs
//! ([`Denoiser`]) and the atlas page edges it may pack into. The bake itself is
//! `rusty-lighting`'s (`lightmap`); the scene stores only these values.

use std::ops::RangeInclusive;

/// The largest atlas page edge, in texels: Unity's largest Max Lightmap Size.
pub const MAX_PAGE: u32 = 4096;
/// The smallest page tried: Unity's smallest Max Lightmap Size.
pub const MIN_PAGE: u32 = 32;
/// The mip levels a page's layout keeps apart (`lightmap::atlas`'s module doc): the
/// base and two halvings.
pub const MIP_LEVELS: u32 = 3;
/// The ring around each lightmap, in texels: the width of a texel of the last
/// mip level.
pub const PAD: u32 = 1 << (MIP_LEVELS - 1);

/// The largest lightmap edge a `page`-edge atlas takes: the page less the ring on
/// both sides, so its cell is exactly the page (`page` is a multiple of the 16-texel grid).
/// The one definition of a map's cap (#1704).
pub const fn max_map_edge(page: u32) -> u32 {
    page - 2 * PAD
}

/// `edge` held to a page edge the packer takes: the next power of two, inside
/// [`MIN_PAGE`]`..=`[`MAX_PAGE`]. What a hand-edited Max Lightmap Size becomes.
pub fn page_edge(edge: u32) -> u32 {
    edge.clamp(MIN_PAGE, MAX_PAGE).next_power_of_two()
}

/// Which denoiser cleans a lightmap's bounce light (Unity's Lighting settings'
/// *Denoiser*). A scene file without one takes [`Denoiser::Oidn`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Denoiser {
    /// Intel Open Image Denoise's `RTLightmap` filter (HDR, trained on lightmaps),
    /// run by the platform, then the Gaussian as under [`Denoiser::Filter`]; where
    /// OIDN is not installed the bake warns and only filters.
    #[default]
    Oidn,
    /// The edge-aware Gaussian, [`BakeSettings::filter_radius`] texels wide.
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

/// The bake's knobs, Unity's Lightmapping settings in miniature. The scene stores
/// them in its lighting settings (#832); a field a scene file omits takes its default.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BakeSettings {
    /// Lightmap texels per world unit (Unity's "Lightmap Resolution").
    pub texels_per_unit: f32,
    /// Paths traced per texel; more is smoother and slower.
    pub samples: u32,
    /// Surfaces a path bounces off before it stops.
    pub bounces: u32,
    /// The seed every texel's sample stream derives from.
    pub seed: u64,
    /// Unity's *Max Lightmap Size* (#1704): the atlas page's edge, in texels, a
    /// power of two in [`Self::MAX_LIGHTMAP_SIZE`]. A mesh's lightmap follows its
    /// area × `texels_per_unit` up to the largest map such a page holds
    /// ([`max_map_edge`]); the packer picks the smallest page that holds the bake,
    /// up to this edge, then adds pages. Read through [`Self::page`].
    pub max_lightmap_size: u32,
    /// What cleans the baked bounce (#1884, Unity's *Denoiser* and *Filter*): OIDN
    /// then the Gaussian below, the Gaussian alone, or nothing. Baked direct light
    /// is never cleaned.
    pub denoiser: Denoiser,
    /// Radius, in texels, of the Gaussian that smooths the baked bounce (Unity's
    /// indirect filter): alone under [`Denoiser::Filter`] or where OIDN cannot run,
    /// after OIDN under `Oidn`; 0 leaves it as traced or as OIDN gave it.
    pub filter_radius: u32,
    /// Also bake each texel's dominant incoming light direction and its `Baked`
    /// lamps' share (#810, Unity's *Directional* mode; #1695), so normal maps
    /// reshape baked light and `Baked` lamps give a lightmapped surface one
    /// specular highlight. Off saves the direction pages' memory; the colour
    /// lightmaps are the same either way.
    pub directional: bool,
    /// Unity's *Backface Tolerance* (#1949): the share of a texel's validity probes
    /// that must reach a front face (or escape) for its light to count. A texel
    /// below it sits inside geometry, and its own chart's valid texels fill it.
    pub backface_tolerance: f32,
    /// Paths a progressive bake traces per texel per pass (#1970, the editor's
    /// Generate Lighting): every texel gets this many, the live scene shows the
    /// result, and the next pass adds as many again, up to [`Self::samples`].
    /// Never changes the bytes, only how often the preview refreshes.
    pub samples_per_pass: u32,
    /// Unity's *Prioritize View* (#1970): after a progressive bake's first pass,
    /// the texels the Scene view's camera sees get all their passes before the
    /// rest. Only when each is traced moves, so never the bytes.
    pub prioritize_view: bool,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            texels_per_unit: 8.0,
            samples: Self::DEFAULT_SAMPLES,
            bounces: 3,
            seed: 0,
            max_lightmap_size: 1024,
            denoiser: Denoiser::Oidn,
            filter_radius: 3,
            directional: true,
            backface_tolerance: Self::DEFAULT_BACKFACE_TOLERANCE,
            samples_per_pass: Self::DEFAULT_SAMPLES_PER_PASS,
            prioritize_view: true,
        }
    }
}

impl BakeSettings {
    /// The default paths per texel: the fewest at which an OIDN bake (OIDN, then
    /// the Gaussian) is as close to a converged one as the Gaussian alone was at
    /// its old default of 128, within a point of mean relative error, in about 60%
    /// of the time (#1884, `dev::bake::denoise::quality_tests`; 32 missed by 1.3 to
    /// 1.7 points). A bake that falls back to the Gaussian alone is blurrier and
    /// blotchier at this count.
    pub const DEFAULT_SAMPLES: u32 = 64;
    /// The default paths per texel per pass of a progressive bake (#1970): eight
    /// passes at the default [`Self::DEFAULT_SAMPLES`], so the first preview shows
    /// after an eighth of the trace and each pass costs one page encode.
    pub const DEFAULT_SAMPLES_PER_PASS: u32 = 8;
    /// Unity's default *Backface Tolerance*: a texel is valid when nine in ten of
    /// its probes see front faces (#1949).
    pub const DEFAULT_BACKFACE_TOLERANCE: f32 = 0.9;
    /// The backface tolerances a bake takes, as Unity's slider.
    pub const BACKFACE_TOLERANCE: RangeInclusive<f32> = 0.0..=1.0;
    /// The texel densities a bake takes (Unity's Lightmap Resolution, in miniature).
    pub const TEXELS_PER_UNIT: RangeInclusive<f32> = 0.25..=64.0;
    /// The samples per texel a bake takes.
    pub const SAMPLES: RangeInclusive<u32> = 1..=4096;
    /// How many passes a progressive bake makes: [`Self::samples`] in steps of
    /// [`Self::samples_per_pass`] (0 held to 1), the last pass taking the rest.
    pub fn passes(&self) -> u32 {
        self.samples.max(1).div_ceil(self.samples_per_pass.max(1))
    }

    /// The bounces a bake takes.
    pub const BOUNCES: RangeInclusive<u32> = 1..=16;
    /// The page edges a bake takes (Unity's 32–4096), each a power of two.
    pub const MAX_LIGHTMAP_SIZE: RangeInclusive<u32> = MIN_PAGE..=MAX_PAGE;

    /// The atlas page edge this bake packs into: [`Self::max_lightmap_size`] held
    /// to a power of two in [`Self::MAX_LIGHTMAP_SIZE`] (a hand-edited file may
    /// hold anything).
    pub fn page(&self) -> u32 {
        page_edge(self.max_lightmap_size)
    }

    /// `tolerance` held to [`Self::BACKFACE_TOLERANCE`], a non-finite one taking
    /// [`Self::DEFAULT_BACKFACE_TOLERANCE`]: the one range rule both
    /// [`Self::sanitized`] and the bake's validity test (`trace::validity`) apply.
    pub fn backface_tolerance_of(tolerance: f32) -> f32 {
        let (low, high) = Self::BACKFACE_TOLERANCE.into_inner();
        match tolerance.is_finite() {
            true => tolerance.clamp(low, high),
            false => Self::DEFAULT_BACKFACE_TOLERANCE,
        }
    }

    /// These settings with each authored knob clamped into its range: the one place
    /// those limits live, applied by `Lighting.SetSettings` / `BakeLightmaps` and
    /// spanned by the editor's fields (#993). A non-finite density or backface
    /// tolerance takes the default.
    pub fn sanitized(self) -> Self {
        let texels = match self.texels_per_unit.is_finite() {
            true => self.texels_per_unit,
            false => Self::default().texels_per_unit,
        };
        let clamp = |v: u32, r: RangeInclusive<u32>| v.clamp(*r.start(), *r.end());
        Self {
            texels_per_unit: texels
                .clamp(*Self::TEXELS_PER_UNIT.start(), *Self::TEXELS_PER_UNIT.end()),
            samples: clamp(self.samples, Self::SAMPLES),
            samples_per_pass: clamp(self.samples_per_pass, Self::SAMPLES),
            bounces: clamp(self.bounces, Self::BOUNCES),
            max_lightmap_size: self.page(),
            backface_tolerance: Self::backface_tolerance_of(self.backface_tolerance),
            ..self
        }
    }
}
