use glam::Vec3;
use serde::{Deserialize, Serialize};

// Fractions `0..=1`: of the sky the clouds cover, and of a cloud's opacity.
pub const DEFAULT_CLOUD_COVERAGE: f32 = 0.5;
pub const DEFAULT_CLOUD_OPACITY: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkySettings {
    pub zenith: Option<Vec3>,
    pub horizon: Option<Vec3>,
    pub ground: Option<Vec3>,
    pub cloud_coverage: f32,
    pub cloud_opacity: f32,
}

impl Default for SkySettings {
    fn default() -> Self {
        Self {
            zenith: None,
            horizon: None,
            ground: None,
            cloud_coverage: DEFAULT_CLOUD_COVERAGE,
            cloud_opacity: DEFAULT_CLOUD_OPACITY,
        }
    }
}

pub(crate) const SKY_HORIZON_TO_WHITE: f32 = 0.6;
pub(crate) const SKY_GROUND_OF_ZENITH: f32 = 0.25;

impl SkySettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Unset colours derive as `common::procedural_sky` derives them: change the two together.
    pub fn resolved(&self, ambient: Vec3) -> [Vec3; 3] {
        let zenith = self.zenith.unwrap_or(ambient);
        let horizon = self
            .horizon
            .unwrap_or(zenith.lerp(Vec3::ONE, SKY_HORIZON_TO_WHITE));
        let ground = self.ground.unwrap_or(zenith * SKY_GROUND_OF_ZENITH);
        [zenith, horizon, ground]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_without_a_sky_block_loads_the_default() {
        let sky: SkySettings = serde_json::from_str("{}").unwrap();
        assert!(sky.is_default());
        let partial: SkySettings = serde_json::from_str(r#"{"cloud_coverage":0}"#).unwrap();
        assert_eq!(partial.cloud_opacity, DEFAULT_CLOUD_OPACITY);
        assert_eq!(partial.zenith, None);
    }

    #[test]
    fn unset_colours_derive_from_the_ambient_then_the_zenith() {
        let ambient = Vec3::new(0.4, 0.5, 0.6);
        let [z, h, g] = SkySettings::default().resolved(ambient);
        assert_eq!((z, g), (ambient, ambient * 0.25));
        assert!(h.abs_diff_eq(Vec3::new(0.76, 0.8, 0.84), 1e-6));
        let set = SkySettings {
            zenith: Some(Vec3::ONE),
            ..SkySettings::default()
        };
        assert_eq!(set.resolved(ambient)[2], Vec3::splat(0.25));
    }
}
