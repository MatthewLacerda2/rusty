use serde::{Deserialize, Serialize};

fn one() -> f32 {
    1.0
}

/// Degrees: a surface turned this far from facing the projector still takes the decal whole.
fn default_angle_fade() -> f32 {
    60.0
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecalBlend {
    #[serde(default = "one")]
    pub albedo: f32,
    #[serde(default = "one")]
    pub normal: f32,
    #[serde(default = "one")]
    pub metallic: f32,
    #[serde(default = "one")]
    pub roughness: f32,
    #[serde(default = "one")]
    pub occlusion: f32,
    #[serde(default = "default_angle_fade")]
    pub angle_fade: f32,
}

impl Default for DecalBlend {
    fn default() -> Self {
        Self {
            albedo: 1.0,
            normal: 1.0,
            metallic: 1.0,
            roughness: 1.0,
            occlusion: 1.0,
            angle_fade: default_angle_fade(),
        }
    }
}

impl DecalBlend {
    /// A decal with no material changes the albedo alone; its 60° is `default_angle_fade`'s.
    pub const ALBEDO_ONLY: Self = Self {
        albedo: 1.0,
        normal: 0.0,
        metallic: 0.0,
        roughness: 0.0,
        occlusion: 1.0,
        angle_fade: 60.0,
    };

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn clamped(self) -> Self {
        let unit = |v: f32| v.clamp(0.0, 1.0);
        Self {
            albedo: unit(self.albedo),
            normal: unit(self.normal),
            metallic: unit(self.metallic),
            roughness: unit(self.roughness),
            occlusion: unit(self.occlusion),
            angle_fade: self.angle_fade.clamp(0.0, 90.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MaterialAsset;

    #[test]
    fn an_omitted_decal_key_takes_its_default() {
        let blend: DecalBlend = serde_json::from_str(r#"{ "normal": 0.0 }"#).unwrap();
        assert_eq!(blend.normal, 0.0);
        assert_eq!(blend.angle_fade, 60.0);
        assert_eq!(blend.albedo, 1.0);
    }

    #[test]
    fn the_decal_fields_are_written_only_when_they_differ_from_the_defaults() {
        let plain = serde_json::to_string(&MaterialAsset::default()).unwrap();
        assert!(!plain.contains("decal"), "{plain}");
        let shy = MaterialAsset {
            receive_decals: false,
            decal: DecalBlend {
                angle_fade: 30.0,
                ..DecalBlend::default()
            },
            ..MaterialAsset::default()
        };
        let json = serde_json::to_string(&shy).unwrap();
        assert!(json.contains(r#""receive_decals":false"#), "{json}");
        assert!(json.contains(r#""angle_fade":30.0"#), "{json}");
        let back: MaterialAsset = serde_json::from_str(&json).unwrap();
        assert!(!back.receive_decals);
        assert_eq!(back.decal, shy.decal);
    }
}
