use glam::Vec4;
use rusty_asset::MapRef;
use serde::{Deserialize, Serialize};

use super::look::{UiBlend, UiGradient};
use super::shader::UiShader;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ImageType {
        #[default]
        Simple = "Simple",
        Sliced = "Sliced",
        Tiled = "Tiled",
        Filled = "Filled",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum FillMethod {
        #[default]
        Horizontal = "Horizontal",
        Vertical = "Vertical",
        Radial360 = "Radial360",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum FillOrigin {
        #[default]
        Left = "Left",
        Right = "Right",
        Bottom = "Bottom",
        Top = "Top",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageComponent {
    pub color: Vec4,
    pub texture: Option<MapRef>,
    pub image_type: ImageType,
    pub border: Vec4,
    pub fill_method: FillMethod,
    pub fill_origin: FillOrigin,
    pub fill_amount: f32,
    pub fill_clockwise: bool,
    pub preserve_aspect: bool,
    pub raycast_target: bool,
    pub gradient: Option<UiGradient>,
    pub blend: UiBlend,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<UiShader>,
    #[serde(skip)]
    pub state_tint: Vec4,
    #[serde(skip)]
    pub override_texture: Option<MapRef>,
}

impl Default for ImageComponent {
    fn default() -> Self {
        Self {
            color: Vec4::ONE,
            texture: None,
            image_type: ImageType::Simple,
            border: Vec4::ZERO,
            fill_method: FillMethod::Horizontal,
            fill_origin: FillOrigin::Left,
            fill_amount: 1.0,
            fill_clockwise: true,
            preserve_aspect: false,
            raycast_target: true,
            gradient: None,
            blend: UiBlend::Normal,
            shader: None,
            state_tint: Vec4::ONE,
            override_texture: None,
        }
    }
}

impl ImageComponent {
    pub fn shown_texture(&self) -> Option<&MapRef> {
        self.override_texture.as_ref().or(self.texture.as_ref())
    }
}

impl crate::FirstClass for ImageComponent {
    const NAME: &'static str = "Image";
}
