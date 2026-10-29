//! Whether and how a mesh casts shadows (#1571): Unity's
//! `MeshRenderer.shadowCastingMode`, honoured by every shadow path (the sun's
//! cascades, the point/spot atlas, their static bakes, and the lightmap bake).

crate::core::named_enum! {
    /// How a mesh takes part in shadows — Unity's *Cast Shadows*. `On` is the
    /// default and is never written to a file.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ShadowCasting {
        /// Casts from its front faces, as every mesh did before the switch.
        #[default]
        On = "On",
        /// Casts nothing, realtime or baked: water, glass, a muzzle flash.
        Off = "Off",
        /// Casts from both faces (culling off in the shadow pass): a single-sided
        /// card or plane that must shadow whichever way it faces the light.
        TwoSided = "TwoSided",
        /// Casts as `On` but is drawn by no camera: an invisible stand-in that only
        /// throws a shadow (a first-person body, a low-poly shadow proxy).
        ShadowsOnly = "ShadowsOnly",
    }
}

impl ShadowCasting {
    /// Whether the mesh is drawn into shadow maps and blocks baked light.
    pub fn casts(self) -> bool {
        self != Self::Off
    }

    /// Whether a camera draws the mesh.
    pub fn is_visible(self) -> bool {
        self != Self::ShadowsOnly
    }

    /// Whether the default, which a file leaves out.
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
