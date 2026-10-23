//! src/scene/material_lookup.rs — resolving an entity's material slots against the
//! scene's library (#1285): the first one (Unity's `sharedMaterial`), the one in a
//! given slot, and all of them.

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

    /// The material (and key) in entity `id`'s material slot `slot` (#1285); `None`
    /// past the end of its list, without a Material, or for a missing key.
    pub fn material_entry_at(&self, id: u32, slot: usize) -> Option<(&str, &MaterialAsset)> {
        let material = self.world.material(id)?;
        let (key, asset) = self
            .materials
            .get_key_value(material.materials.get(slot)?)?;
        Some((key.as_str(), asset))
    }

    /// Every library material entity `id` references, in slot order.
    pub fn material_assets_of(&self, id: u32) -> impl Iterator<Item = &MaterialAsset> {
        let count = self.world.material(id).map_or(0, |m| m.materials.len());
        (0..count).filter_map(move |slot| self.material_entry_at(id, slot).map(|(_, a)| a))
    }
}
