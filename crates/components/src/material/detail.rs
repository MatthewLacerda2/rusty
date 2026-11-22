use serde::{Deserialize, Serialize};

use rusty_asset::MapRef;

use super::mapping::default_tiling;
use super::surface::default_one;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailLayer {
    #[serde(default)]
    pub map: Option<MapRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<MapRef>,
    #[serde(default = "default_tiling")]
    pub tiling: [f32; 2],
    #[serde(default)]
    pub offset: [f32; 2],
    #[serde(default = "default_one")]
    pub world_size: f32,
    #[serde(default = "default_one")]
    pub albedo_scale: f32,
    #[serde(default = "default_one")]
    pub normal_scale: f32,
    #[serde(default = "default_one")]
    pub smoothness_scale: f32,
}

impl DetailLayer {
    pub fn map_only(&self) -> Option<Self> {
        let map = self.map.clone()?;
        Some(Self {
            map: Some(map),
            mask: self.mask.clone(),
            ..Self::default()
        })
    }
}

impl Default for DetailLayer {
    fn default() -> Self {
        Self {
            map: None,
            mask: None,
            tiling: default_tiling(),
            offset: [0.0; 2],
            world_size: default_one(),
            albedo_scale: default_one(),
            normal_scale: default_one(),
            smoothness_scale: default_one(),
        }
    }
}
