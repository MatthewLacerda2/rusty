rusty_core::named_enum! {
    /// How a mesh takes part in shadows — Unity's *Cast Shadows*. `On` is the
    /// default, and what a file without the field reads.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ShadowCasting {
        #[default]
        On = "On",
        /// Casts nothing, realtime or baked; the bake still sees it and lightmaps it.
        Off = "Off",
        /// Casts from both faces (culling off in the shadow pass): a single-sided
        /// card or plane that must shadow whichever way it faces the light.
        TwoSided = "TwoSided",
        /// Casts as `On`, but no camera draws it and no bake sees it: a shadow-only stand-in.
        ShadowsOnly = "ShadowsOnly",
    }
}

impl ShadowCasting {
    pub fn casts(self) -> bool {
        self != Self::Off
    }

    pub fn is_visible(self) -> bool {
        self != Self::ShadowsOnly
    }

    pub fn is_on(&self) -> bool {
        *self == Self::On
    }
}

#[cfg(test)]
mod tests {
    use super::ShadowCasting;

    #[test]
    fn modes_say_who_draws_them() {
        let casts = ShadowCasting::ALL.map(ShadowCasting::casts);
        assert_eq!(casts, [true, false, true, true]);
        let seen = ShadowCasting::ALL.map(ShadowCasting::is_visible);
        assert_eq!(seen, [true, true, true, false]);
        assert!(ShadowCasting::default().is_on());
    }

    #[test]
    fn modes_are_stored_by_unity_name() {
        let json = serde_json::to_string(&ShadowCasting::ShadowsOnly).unwrap();
        assert_eq!(json, "\"ShadowsOnly\"");
        let read: ShadowCasting = serde_json::from_str("\"twosided\"").unwrap();
        assert_eq!(read, ShadowCasting::TwoSided);
    }
}
