use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaskComponent {
    pub show_mask_graphic: bool,
}

impl Default for MaskComponent {
    fn default() -> Self {
        Self {
            show_mask_graphic: true,
        }
    }
}

impl crate::FirstClass for MaskComponent {
    const NAME: &'static str = "Mask";
}
