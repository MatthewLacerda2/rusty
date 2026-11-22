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
    pub fn gpu(self) -> f32 {
        match self {
            Self::Auto => -1.0,
            Self::Off => 0.0,
            Self::Full => MAX_REACTIVE,
        }
    }
}

pub const MAX_REACTIVE: f32 = 0.9;
