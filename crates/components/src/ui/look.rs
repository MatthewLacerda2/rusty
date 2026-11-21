use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum UiBlend {
        #[default]
        Normal = "Normal",
        Additive = "Additive",
        Multiply = "Multiply",
        Screen = "Screen",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum GradientKind {
        #[default]
        Linear = "Linear",
        Radial = "Radial",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub t: f32,
    pub color: Vec4,
}

pub const MAX_GRADIENT_STOPS: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiGradient {
    pub kind: GradientKind,
    pub angle: f32,
    pub center: Vec2,
    pub radius: f32,
    pub stops: Vec<GradientStop>,
}

impl Default for UiGradient {
    fn default() -> Self {
        Self {
            kind: GradientKind::Linear,
            angle: 0.0,
            center: Vec2::splat(0.5),
            radius: 0.5,
            stops: vec![
                GradientStop {
                    t: 0.0,
                    color: Vec4::ONE,
                },
                GradientStop {
                    t: 1.0,
                    color: Vec4::new(0.0, 0.0, 0.0, 1.0),
                },
            ],
        }
    }
}
