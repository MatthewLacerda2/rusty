//! src/core/quality.rs — `QualityPreset`: the global scalability tier.
//!
//! One engine-wide resource (Low / Medium / High) that the `Graphics` API and the
//! editor header write and the renderer's post-FX chain reads to decide which
//! passes run and how large the bloom buffers are. Plain data with its per-tier
//! decisions, no GPU types — the render layer maps it onto its passes (#494).

/// Scalability tier. Gates SSR + motion blur and the bloom buffer size.
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

    /// Divisor applied to the bloom buffer resolution (smaller = cheaper).
    pub fn bloom_divisor(self) -> u32 {
        match self {
            QualityPreset::Low => 4,
            QualityPreset::Medium | QualityPreset::High => 2,
        }
    }
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
}
