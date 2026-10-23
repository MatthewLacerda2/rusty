//! src/scene/material_lookup.rs — resolving an entity's material slots against the
//! scene's library (#1285): the first one (Unity's `sharedMaterial`), the one a mesh
//! run draws with, and all of them.

use crate::scene::{MaterialAsset, Scene};

impl Scene {
    /// Entity `id`'s first material; `None` without a Material or for a missing key.
    pub fn material_asset_of(&self, id: u32) -> Option<&MaterialAsset> {
        self.material_entry_of(id).map(|(_, asset)| asset)
    }

    /// Entity `id`'s first material with its library key (the renderer keys a
    /// material's runtime shader params by it, #399).
    pub fn material_entry_of(&self, id: u32) -> Option<(&str, &MaterialAsset)> {
        self.material_entry_at(id, 0)
    }

    /// The material (and key) entity `id`'s mesh run `range` draws with (#1285).
    pub fn material_entry_at(&self, id: u32, range: usize) -> Option<(&str, &MaterialAsset)> {
        let key = self.world.material(id)?.for_range(range).to_string();
        let (key, asset) = self.materials.get_key_value(&key)?;
        Some((key.as_str(), asset))
    }

    /// Every library material entity `id` references, in slot order.
    pub fn material_assets_of(&self, id: u32) -> Vec<&MaterialAsset> {
        let keys = self.world.material(id).map(|m| m.materials.clone());
        keys.unwrap_or_default()
            .iter()
            .filter_map(|k| self.materials.get(k))
            .collect()
    }
}
