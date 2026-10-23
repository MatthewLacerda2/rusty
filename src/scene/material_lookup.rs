//! src/scene/material_lookup.rs — resolving an entity's material slots against the
//! scene's library (#1285): the first one (Unity's `sharedMaterial`), one by slot,
//! all of them, and which slot draws which mesh run.

use crate::scene::{MaterialAsset, Scene};

/// A library material and its key, as the renderer takes it.
pub type MaterialEntry<'a> = (&'a str, &'a MaterialAsset);

/// One draw of an entity's mesh: material slot `slot` over mesh run `run`, with the
/// library material it resolves to (`None`: no Material, or a key the library lacks,
/// which draw with the engine default).
#[derive(Clone, Copy, Debug)]
pub struct SlotDraw<'a> {
    pub slot: usize,
    pub run: usize,
    pub entry: Option<MaterialEntry<'a>>,
}

impl Scene {
    /// Entity `id`'s first material; `None` without a Material or for a missing key.
    pub fn material_asset_of(&self, id: u32) -> Option<&MaterialAsset> {
        self.material_entry_of(id).map(|(_, asset)| asset)
    }

    /// Entity `id`'s first material with its library key (the renderer keys a
    /// material's runtime shader params by it, #399).
    pub fn material_entry_of(&self, id: u32) -> Option<MaterialEntry<'_>> {
        self.material_entry_at(id, 0)
    }

    /// Entity `id`'s material in slot `slot`, with its key; `None` past its slots.
    pub fn material_entry_at(&self, id: u32, slot: usize) -> Option<MaterialEntry<'_>> {
        let material = self.world.material(id)?;
        let (key, asset) = self
            .materials
            .get_key_value(material.materials.get(slot)?)?;
        Some((key.as_str(), asset))
    }

    /// Every library material entity `id` references, in slot order.
    pub fn material_assets_of(&self, id: u32) -> Vec<&MaterialAsset> {
        let Some(material) = self.world.material(id) else {
            return Vec::new();
        };
        let assets = material
            .materials
            .iter()
            .filter_map(|k| self.materials.get(k));
        assets.collect()
    }

    /// The draws entity `id`'s mesh of `runs` runs makes (#1285): one per slot, over
    /// the run [`MaterialComponent::slot_runs`] gives it, Unity's way. With no
    /// Material every run draws once with the engine default.
    ///
    /// [`MaterialComponent::slot_runs`]: crate::components::MaterialComponent::slot_runs
    pub fn material_draws(&self, id: u32, runs: usize) -> Vec<SlotDraw<'_>> {
        let Some(material) = self.world.material(id) else {
            let bare = |run| SlotDraw {
                slot: run,
                run,
                entry: None,
            };
            return (0..runs).map(bare).collect();
        };
        let entry = |slot: usize| {
            let (key, asset) = self.materials.get_key_value(&material.materials[slot])?;
            Some((key.as_str(), asset))
        };
        let draws = material.slot_runs(runs);
        draws
            .map(|(slot, run)| SlotDraw {
                slot,
                run,
                entry: entry(slot),
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "material_lookup_tests.rs"]
mod tests;
