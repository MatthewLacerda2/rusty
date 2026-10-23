//! The Material card's slot list (#1285): one row per sub-mesh run of the entity's
//! mesh (Unity's `Materials` list, `Element 0`, `Element 1`, …). Each row picks the
//! library material that run draws with, through the shared
//! `authoring::material::set_slot` op `Material.SetAt` calls too, and the selected
//! row is the one the card below edits. A one-slot entity shows no list.

use std::collections::BTreeMap;

use crate::ecs::World;
use crate::scene::authoring::material as mat_ops;
use crate::scene::MaterialAsset;

/// Draw entity `id`'s slot list when it has more than one slot, and return the key
/// of the slot the card edits (the first when there is no list). `None` when the
/// entity has no Material.
pub(super) fn draw_slots(
    ui: &mut egui::Ui,
    world: &mut World,
    id: u32,
    materials: &BTreeMap<String, MaterialAsset>,
    is_dirty: &mut bool,
) -> Option<String> {
    let keys = mat_ops::keys(world, id);
    if keys.len() <= 1 {
        return keys.into_iter().next();
    }
    let memory = egui::Id::new(("material_slot", id));
    let mut selected: usize = ui.data_mut(|d| d.get_temp(memory).unwrap_or(0));
    selected = selected.min(keys.len() - 1);
    ui.label("Materials:");
    for (slot, key) in keys.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.radio_value(&mut selected, slot, format!("Element {slot}"));
            egui::ComboBox::from_id_salt(("material_slot_pick", id, slot))
                .selected_text(key.as_str())
                .show_ui(ui, |ui| {
                    for name in materials.keys() {
                        if ui.selectable_label(name == key, name.as_str()).clicked()
                            && mat_ops::set_slot(world, materials, id, slot, name).is_ok()
                        {
                            *is_dirty = true;
                        }
                    }
                });
        });
    }
    ui.data_mut(|d| d.insert_temp(memory, selected));
    keys.into_iter().nth(selected)
}
