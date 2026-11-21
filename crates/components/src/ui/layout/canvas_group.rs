use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasGroupComponent {
    pub alpha: f32,
    pub interactable: bool,
    pub blocks_raycasts: bool,
}

impl Default for CanvasGroupComponent {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            interactable: true,
            blocks_raycasts: true,
        }
    }
}

impl crate::FirstClass for CanvasGroupComponent {
    const NAME: &'static str = "CanvasGroup";
}
