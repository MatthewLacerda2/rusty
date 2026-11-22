use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LayoutAxisFit {
        #[default]
        Unconstrained = "Unconstrained",
        MinSize = "MinSize",
        PreferredSize = "PreferredSize",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutElementComponent {
    pub ignore_layout: bool,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub preferred_width: Option<f32>,
    pub preferred_height: Option<f32>,
    pub flexible_width: Option<f32>,
    pub flexible_height: Option<f32>,
    pub horizontal_fit: LayoutAxisFit,
    pub vertical_fit: LayoutAxisFit,
}

impl LayoutElementComponent {
    pub fn overrides(&self, axis: usize) -> [Option<f32>; 3] {
        if axis == 0 {
            [self.min_width, self.preferred_width, self.flexible_width]
        } else {
            [self.min_height, self.preferred_height, self.flexible_height]
        }
    }

    pub fn fit(&self, axis: usize) -> LayoutAxisFit {
        [self.horizontal_fit, self.vertical_fit][axis]
    }
}

impl crate::FirstClass for LayoutElementComponent {
    const NAME: &'static str = "LayoutElement";
    const ALIASES: &'static [&'static str] = &["contentsizefitter"];
}
