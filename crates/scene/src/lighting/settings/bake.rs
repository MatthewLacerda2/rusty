use std::ops::RangeInclusive;

// Atlas page edges, texels: Unity's Max Lightmap Size range.
pub const MAX_PAGE: u32 = 4096;
pub const MIN_PAGE: u32 = 32;
pub fn page_edge(edge: u32) -> u32 {
    edge.clamp(MIN_PAGE, MAX_PAGE).next_power_of_two()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Denoiser {
    #[default]
    Oidn,
    Filter,
    None,
}

impl Denoiser {
    pub const ALL: [Denoiser; 3] = [Denoiser::Oidn, Denoiser::Filter, Denoiser::None];

    pub fn name(self) -> &'static str {
        match self {
            Denoiser::Oidn => "OIDN",
            Denoiser::Filter => "Filter",
            Denoiser::None => "None",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|d| d.name().eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BakeSettings {
    pub texels_per_unit: f32,
    pub samples: u32,
    pub direct_samples: u32,
    pub bounces: u32,
    pub seed: u64,
    pub max_lightmap_size: u32,
    pub denoiser: Denoiser,
    pub filter_radius: u32,
    pub directional: bool,
    pub backface_tolerance: f32,
    pub samples_per_pass: u32,
    pub prioritize_view: bool,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            texels_per_unit: 8.0,
            samples: Self::DEFAULT_SAMPLES,
            direct_samples: 0,
            bounces: Self::DEFAULT_BOUNCES,
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
    /// Paths per texel: the fewest at which OIDN then the Gaussian matches the Gaussian alone
    /// at 128 (`denoise::quality_tests`, run by hand).
    pub const DEFAULT_SAMPLES: u32 = 64;
    pub const DEFAULT_SAMPLES_PER_PASS: u32 = 8;
    pub const DEFAULT_BACKFACE_TOLERANCE: f32 = 0.9;
    pub const BACKFACE_TOLERANCE: RangeInclusive<f32> = 0.0..=1.0;
    pub const TEXELS_PER_UNIT: RangeInclusive<f32> = 0.25..=64.0;
    pub const SAMPLES: RangeInclusive<u32> = 1..=4096;
    pub const DIRECT_SAMPLES: RangeInclusive<u32> = 0..=4096;

    pub fn direct_sample_count(&self) -> u32 {
        match self.direct_samples {
            0 => self.samples,
            n => n,
        }
    }

    pub fn passes(&self) -> u32 {
        self.samples.max(1).div_ceil(self.samples_per_pass.max(1))
    }

    pub const DEFAULT_BOUNCES: u32 = 5;
    pub const BOUNCES: RangeInclusive<u32> = 1..=16;
    pub const MAX_LIGHTMAP_SIZE: RangeInclusive<u32> = MIN_PAGE..=MAX_PAGE;

    pub fn page(&self) -> u32 {
        page_edge(self.max_lightmap_size)
    }

    /// The one range rule both `sanitized` and the bake's validity test apply.
    pub fn backface_tolerance_of(tolerance: f32) -> f32 {
        let (low, high) = Self::BACKFACE_TOLERANCE.into_inner();
        match tolerance.is_finite() {
            true => tolerance.clamp(low, high),
            false => Self::DEFAULT_BACKFACE_TOLERANCE,
        }
    }

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
            direct_samples: clamp(self.direct_samples, Self::DIRECT_SAMPLES),
            samples_per_pass: clamp(self.samples_per_pass, Self::SAMPLES),
            bounces: clamp(self.bounces, Self::BOUNCES),
            max_lightmap_size: self.page(),
            backface_tolerance: Self::backface_tolerance_of(self.backface_tolerance),
            ..self
        }
    }
}
