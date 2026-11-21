use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavMeshModifierComponent {
    pub active: bool,
    pub override_area: Option<u8>,
    pub ignore_from_build: bool,
    pub apply_to_children: bool,
}

impl Default for NavMeshModifierComponent {
    fn default() -> Self {
        Self {
            active: true,
            override_area: None,
            ignore_from_build: false,
            apply_to_children: true,
        }
    }
}

impl crate::FirstClass for NavMeshModifierComponent {
    const NAME: &'static str = "NavMeshModifier";
    const ALIASES: &'static [&'static str] = &["navmodifier"];
}
