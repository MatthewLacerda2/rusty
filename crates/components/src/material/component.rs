use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub materials: Vec<String>,
}

impl MaterialComponent {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            materials: vec![key.into()],
        }
    }

    pub fn for_entity(id: u32) -> Self {
        Self::new(entity_material_key(id))
    }

    /// `""` for an empty list, a key no library entry has.
    pub fn first(&self) -> &str {
        self.materials.first().map_or("", String::as_str)
    }
}

/// The one spelling of an entity's own material key: every path creating one calls this.
pub fn entity_material_key(id: u32) -> String {
    format!("entity_{id}_material")
}

impl crate::FirstClass for MaterialComponent {
    const NAME: &'static str = "Material";
}
