use crate::{MaterialAsset, MaterialComponent, Scene};

impl Scene {
    /// A library asset already under the key is kept, never overwritten: every attach path's rule.
    pub fn ensure_material_key(&mut self, id: u32) -> Option<String> {
        if !self.world.contains(id) {
            return None;
        }
        if !self.world.has_material(id) {
            self.world
                .set_material(id, Some(MaterialComponent::for_entity(id)));
        }
        let key = self.world.material(id)?.first().to_string();
        self.materials.entry(key.clone()).or_default();
        Some(key)
    }

    pub fn material_asset_of(&self, id: u32) -> Option<&MaterialAsset> {
        self.material_entry_of(id).map(|(_, asset)| asset)
    }

    pub fn material_entry_of(&self, id: u32) -> Option<(&str, &MaterialAsset)> {
        self.material_entry_at(id, 0)
    }

    pub fn material_entry_at(&self, id: u32, slot: usize) -> Option<(&str, &MaterialAsset)> {
        let material = self.world.material(id)?;
        let (key, asset) = self
            .materials
            .get_key_value(material.materials.get(slot)?)?;
        Some((key.as_str(), asset))
    }

    pub fn material_assets_of(&self, id: u32) -> impl Iterator<Item = &MaterialAsset> {
        let count = self.world.material(id).map_or(0, |m| m.materials.len());
        (0..count).filter_map(move |slot| self.material_entry_at(id, slot).map(|(_, a)| a))
    }
}
