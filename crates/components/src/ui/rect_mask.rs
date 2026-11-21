use glam::Vec4;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RectMaskComponent {
    pub padding: Vec4,
    pub feather: f32,
}

impl crate::FirstClass for RectMaskComponent {
    const NAME: &'static str = "RectMask";
    const ALIASES: &'static [&'static str] = &["rectmask2d"];
}
