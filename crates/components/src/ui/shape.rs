use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

use super::look::{UiBlend, UiGradient};
use super::shader::UiShader;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ShapeKind {
        #[default]
        Rect = "Rect",
        Ellipse = "Ellipse",
        Ring = "Ring",
        Line = "Line",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ShapeCorner {
        #[default]
        Round = "Round",
        Chamfer = "Chamfer",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeShadow {
    pub offset: Vec2,
    pub blur: f32,
    pub color: Vec4,
}

impl Default for ShapeShadow {
    fn default() -> Self {
        Self {
            offset: Vec2::new(4.0, -4.0),
            blur: 4.0,
            color: Vec4::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeGlow {
    pub size: f32,
    pub intensity: f32,
    pub color: Vec4,
}

impl Default for ShapeGlow {
    fn default() -> Self {
        Self {
            size: 0.0,
            intensity: 1.0,
            color: Vec4::ZERO,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeComponent {
    pub kind: ShapeKind,
    pub corner: ShapeCorner,
    pub radius: Vec4,
    pub inner_radius: f32,
    pub arc_start: f32,
    pub arc_end: f32,
    pub thickness: f32,
    pub dash: f32,
    pub gap: f32,
    pub color: Vec4,
    pub gradient: Option<UiGradient>,
    pub border_width: f32,
    pub border_color: Vec4,
    pub shadow: ShapeShadow,
    pub glow: ShapeGlow,
    pub blend: UiBlend,
    pub raycast_target: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<UiShader>,
    #[serde(skip)]
    pub state_tint: Vec4,
}

impl Default for ShapeComponent {
    fn default() -> Self {
        Self {
            kind: ShapeKind::Rect,
            corner: ShapeCorner::Round,
            radius: Vec4::ZERO,
            inner_radius: 0.0,
            arc_start: 0.0,
            arc_end: 360.0,
            thickness: 2.0,
            dash: 0.0,
            gap: 0.0,
            color: Vec4::ONE,
            gradient: None,
            border_width: 0.0,
            border_color: Vec4::ONE,
            shadow: ShapeShadow::default(),
            glow: ShapeGlow::default(),
            blend: UiBlend::Normal,
            raycast_target: true,
            shader: None,
            state_tint: Vec4::ONE,
        }
    }
}

impl ShapeShadow {
    pub fn is_on(&self) -> bool {
        self.color.w > 0.0
    }
}

impl ShapeGlow {
    pub fn is_on(&self) -> bool {
        self.size > 0.0 && self.color.w > 0.0 && self.intensity > 0.0
    }
}

impl crate::FirstClass for ShapeComponent {
    const NAME: &'static str = "Shape";
}
