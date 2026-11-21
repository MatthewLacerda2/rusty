use serde::{Deserialize, Serialize};

pub const LAYER_COUNT: usize = 32;

const DEFAULT_LAYER_NAME: &str = "Default";

pub fn layer_in_mask(layer: u8, mask: u32) -> bool {
    (layer as usize) < LAYER_COUNT && (mask & (1u32 << layer)) != 0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerRegistry {
    names: Vec<String>,
}

impl Default for LayerRegistry {
    fn default() -> Self {
        let mut names = vec![String::new(); LAYER_COUNT];
        names[0] = DEFAULT_LAYER_NAME.to_string();
        Self { names }
    }
}

impl LayerRegistry {
    pub fn name(&self, index: u8) -> &str {
        self.names
            .get(index as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn label(&self, index: u8) -> String {
        let name = self.name(index);
        if name.is_empty() {
            format!("Layer {index}")
        } else {
            name.to_string()
        }
    }

    pub fn set_name(&mut self, index: u8, name: impl Into<String>) {
        if index == 0 {
            return;
        }
        if let Some(slot) = self.names.get_mut(index as usize) {
            *slot = name.into().trim().to_string();
        }
    }

    pub fn index_of(&self, name: &str) -> Option<u8> {
        if name.is_empty() {
            return None;
        }
        self.names.iter().position(|n| n == name).map(|i| i as u8)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u8, &str)> {
        self.names
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u8, n.as_str()))
    }

    pub fn normalize(&mut self) {
        self.names.resize(LAYER_COUNT, String::new());
        self.names[0] = DEFAULT_LAYER_NAME.to_string();
    }
}
