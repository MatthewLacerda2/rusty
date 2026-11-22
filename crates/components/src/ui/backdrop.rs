use glam::Vec4;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackdropFilterComponent {
    pub blur_radius: f32,
    pub tint: Vec4,
    pub saturation: f32,
    pub brightness: f32,
}

impl Default for BackdropFilterComponent {
    fn default() -> Self {
        Self {
            blur_radius: 16.0,
            tint: Vec4::ZERO,
            saturation: 1.0,
            brightness: 1.0,
        }
    }
}

impl crate::FirstClass for BackdropFilterComponent {
    const NAME: &'static str = "BackdropFilter";
    const ALIASES: &'static [&'static str] = &["backdrop"];
}
