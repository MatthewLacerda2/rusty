use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

use super::look::UiBlend;
use super::shader::UiShader;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextAlignment {
        #[default]
        TopLeft = "TopLeft",
        TopCenter = "TopCenter",
        TopRight = "TopRight",
        MiddleLeft = "MiddleLeft",
        MiddleCenter = "MiddleCenter",
        MiddleRight = "MiddleRight",
        BottomLeft = "BottomLeft",
        BottomCenter = "BottomCenter",
        BottomRight = "BottomRight",
    }
}

impl TextAlignment {
    pub fn fractions(self) -> Vec2 {
        let i = Self::ALL.iter().position(|a| *a == self).unwrap_or(0);
        Vec2::new((i % 3) as f32 * 0.5, 1.0 - (i / 3) as f32 * 0.5)
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextOverflow {
        #[default]
        Overflow = "Overflow",
        Truncate = "Truncate",
        Ellipsis = "Ellipsis",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextComponent {
    pub text: String,
    pub font: Option<String>,
    pub font_bold: Option<String>,
    pub font_italic: Option<String>,
    pub font_size: f32,
    pub color: Vec4,
    pub alignment: TextAlignment,
    pub wrap: bool,
    pub overflow: TextOverflow,
    pub line_spacing: f32,
    pub letter_spacing: f32,
    pub auto_size: bool,
    pub auto_size_min: f32,
    pub auto_size_max: f32,
    pub rich_text: bool,
    pub raycast_target: bool,
    pub outline_width: f32,
    pub outline_color: Vec4,
    pub shadow_offset: Vec2,
    pub shadow_color: Vec4,
    pub glow_size: f32,
    pub glow_color: Vec4,
    pub blend: UiBlend,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<UiShader>,
    #[serde(skip)]
    pub state_tint: Vec4,
}

impl Default for TextComponent {
    fn default() -> Self {
        Self {
            text: "New Text".into(),
            font: None,
            font_bold: None,
            font_italic: None,
            font_size: 36.0,
            color: Vec4::new(0.196, 0.196, 0.196, 1.0),
            alignment: TextAlignment::TopLeft,
            wrap: true,
            overflow: TextOverflow::Overflow,
            line_spacing: 1.0,
            letter_spacing: 0.0,
            auto_size: false,
            auto_size_min: 10.0,
            auto_size_max: 72.0,
            rich_text: true,
            raycast_target: true,
            outline_width: 0.0,
            outline_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            shadow_offset: Vec2::ZERO,
            shadow_color: Vec4::new(0.0, 0.0, 0.0, 0.5),
            glow_size: 0.0,
            glow_color: Vec4::new(0.0, 1.0, 1.0, 0.75),
            blend: UiBlend::Normal,
            shader: None,
            state_tint: Vec4::ONE,
        }
    }
}

impl crate::FirstClass for TextComponent {
    const NAME: &'static str = "Text";
    const ALIASES: &'static [&'static str] = &["textmeshpro", "textmeshprougui"];
}
