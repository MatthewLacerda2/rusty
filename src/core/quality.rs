//! src/core/quality.rs — `QualityPreset`: the global scalability tier.
//!
//! One engine-wide resource (Low / Medium / High) that the `Graphics` API and the
//! editor header write and the renderer's post-FX chain reads to decide which
//! passes run and how large the bloom buffers are. Plain data with its per-tier
//! decisions, no GPU types — the render layer maps it onto its passes (#494).

/// Scalability tier. Gates SSR, motion blur and SSAO, and the bloom buffer size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QualityPreset {
    /// iGPU floor: SSR off (cubemap only), motion blur off, quarter-res bloom.
    Low,
    /// Balanced: cubemap reflection, motion blur on, half-res bloom.
    #[default]
    Medium,
    /// Discrete GPU: real screen-space reflections, motion blur, half-res bloom.
    High,
}

impl QualityPreset {
    /// Does this tier run the real screen-space-reflection pass?
    pub fn screen_space_ssr(self) -> bool {
        matches!(self, QualityPreset::High)
    }

    /// Does this tier run the camera motion-blur pass?
    pub fn motion_blur(self) -> bool {
        !matches!(self, QualityPreset::Low)
    }

    /// How the SSAO pass runs on this tier (#436), or `None` where it is off: Low
    /// skips it, Medium traces 8 samples at half resolution, High 16 at full.
    pub fn ssao(self) -> Option<SsaoTier> {
        match self {
            QualityPreset::Low => None,
            QualityPreset::Medium => Some(SsaoTier {
                divisor: 2,
                samples: 8,
            }),
            QualityPreset::High => Some(SsaoTier {
                divisor: 1,
                samples: 16,
            }),
        }
    }

    /// Divisor applied to the bloom buffer resolution (smaller = cheaper).
    pub fn bloom_divisor(self) -> u32 {
        match self {
            QualityPreset::Low => 4,
            QualityPreset::Medium | QualityPreset::High => 2,
        }
    }

    /// Divisor of the UI backdrop blur's working resolution (#426) — a power of
    /// two, the size its first level is composited at. The bloom's, for the same
    /// reason: a blurred image loses nothing to a smaller buffer.
    pub fn backdrop_divisor(self) -> u32 {
        self.bloom_divisor()
    }
}

/// The resolution and sample count of the SSAO pass on one tier (#436).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SsaoTier {
    /// The occlusion target is the view's size divided by this (1 or 2).
    pub divisor: u32,
    /// Hemisphere samples traced per occlusion texel.
    pub samples: u32,
}

#[cfg(test)]
mod tests {
    use super::QualityPreset;

    /// `Renderer::set_quality` reallocates the bloom buffers only when the new tier
    /// changes their resolution divisor. This pins the divisor-per-tier decision —
    /// the GPU-free half of that reallocation — so a live preset switch resizes
    /// correctly. (The actual texture realloc needs a device and can't run
    /// headless; this asserts what reconfiguration each tier requests.)
    #[test]
    fn bloom_divisor_changes_with_tier() {
        assert_eq!(QualityPreset::Low.bloom_divisor(), 4);
        assert_eq!(QualityPreset::Medium.bloom_divisor(), 2);
        assert_eq!(QualityPreset::High.bloom_divisor(), 2);

        // Low<->Medium crosses a divisor boundary => a switch must realloc.
        assert_ne!(
            QualityPreset::Low.bloom_divisor(),
            QualityPreset::Medium.bloom_divisor()
        );
        // Medium<->High keeps the same bloom size => realloc can be skipped.
        assert_eq!(
            QualityPreset::Medium.bloom_divisor(),
            QualityPreset::High.bloom_divisor()
        );
    }

    /// SSR is the High-tier-only pass; motion blur is off only on Low. A live
    /// preset switch toggles these passes, so the gating decisions are pinned here.
    #[test]
    fn ssr_is_high_only_and_motion_blur_off_only_on_low() {
        assert!(!QualityPreset::Low.screen_space_ssr());
        assert!(!QualityPreset::Medium.screen_space_ssr());
        assert!(QualityPreset::High.screen_space_ssr());

        assert!(!QualityPreset::Low.motion_blur());
        assert!(QualityPreset::Medium.motion_blur());
        assert!(QualityPreset::High.motion_blur());
    }

    /// SSAO (#436): off on Low, half resolution on Medium, full on High.
    #[test]
    fn ssao_is_off_on_low_and_sharper_on_high() {
        assert_eq!(QualityPreset::Low.ssao(), None);
        let (m, h) = (QualityPreset::Medium.ssao(), QualityPreset::High.ssao());
        assert_eq!(m.map(|t| t.divisor), Some(2));
        assert_eq!(h.map(|t| t.divisor), Some(1));
        assert!(h.unwrap().samples > m.unwrap().samples);
    }
}
