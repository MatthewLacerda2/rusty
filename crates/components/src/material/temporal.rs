rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TaaReactive {
        #[default]
        Auto = "Auto",
        Off = "Off",
        Full = "Full",
    }
}

impl TaaReactive {
    /// Below zero is `Auto`: `reactive.wgsl`'s `texel` reads only `>= 0` as a fixed value.
    pub fn gpu(self) -> f32 {
        match self {
            Self::Auto => -1.0,
            Self::Off => 0.0,
            Self::Full => MAX_REACTIVE,
        }
    }
}

/// FSR2's advice: clamp the reactive mask to about 0.9, as near 1 never looks good. Mirrors
/// `reactive.wgsl`'s `MAX_REACTIVE`.
pub const MAX_REACTIVE: f32 = 0.9;
