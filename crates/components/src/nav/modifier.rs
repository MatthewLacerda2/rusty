use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavMeshModifierVolumeComponent {
    pub active: bool,
    pub center: Vec3,
    pub size: Vec3,
    pub area: u8,
}

impl Default for NavMeshModifierVolumeComponent {
    fn default() -> Self {
        Self {
            active: true,
            center: Vec3::new(0.0, 1.0, 0.0),
            size: Vec3::new(4.0, 3.0, 4.0),
            area: 0,
        }
    }
}

impl crate::FirstClass for NavMeshModifierVolumeComponent {
    const NAME: &'static str = "NavMeshModifierVolume";
    const ALIASES: &'static [&'static str] = &["navmodifiervolume"];
}
