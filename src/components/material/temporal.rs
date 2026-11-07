//! How a transparent surface or a sprite emitter takes part in temporal
//! anti-aliasing (#1845): its *reactivity*, FSR2's term for how much of a pixel's
//! colour comes from this frame rather than the history. A moving translucent
//! writes no motion of its own, so TAA reprojects it with the surface behind it;
//! a reactive pixel trusts this frame more and keeps the translucent crisp.

crate::core::named_enum! {
    /// A transparent material's or a particle emitter's reactivity under TAA
    /// (#1845): FSR2's *reactive mask*, UE's *Responsive AA*. Read only where the
    /// view runs TAA.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TaaReactive {
        /// From what the surface adds to the pixel: its alpha when it blends over
        /// what is behind (FSR2's recommendation), its brightness when it adds to
        /// it. The default.
        #[default]
        Auto = "Auto",
        /// Never: the history smooths it as it does an opaque surface. For a large
        /// still window, whose edges would otherwise shimmer with the jitter.
        Off = "Off",
        /// Always as reactive as TAA lets a pixel be (FSR2's 0.9): for a fast
        /// translucent whose detail must not trail (a plasma ball, a muzzle flash).
        Full = "Full",
    }
}

impl TaaReactive {
    /// The value the GPU reads: below zero for `Auto`, else the fixed reactivity.
    pub fn gpu(self) -> f32 {
        match self {
            Self::Auto => -1.0,
            Self::Off => 0.0,
            Self::Full => MAX_REACTIVE,
        }
    }
}

/// The most reactive a pixel gets: FSR2's README recommends clamping the reactive
/// mask "to around 0.9", since a value near 1 never produces good results.
pub const MAX_REACTIVE: f32 = 0.9;
