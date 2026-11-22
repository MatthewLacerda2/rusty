//! A THIN client: widgets read a snapshot and route every write through the
//! shared `authoring::nav::modifier` ops the `NavMeshModifierVolume.*` Lua setters
//! call. A volume assigns areas at bake time: the editor's per-frame nav sync
//! rebakes only the volume's cells after an edit.

use crate::inspector::components::card::remove_component;
use crate::widgets::DragRange;
use egui_phosphor::regular as icon;
use rusty_edit::authoring::ComponentKind;

use super::super::physics::vec3_row;
use crate::inspector::components::card::component_card;
use rusty_components::NavMeshModifierVolumeComponent as Volume;
use rusty_edit::authoring::nav::modifier as volume_ops;
use rusty_navigation::MAX_AREAS;

/// Route one edit through the shared ops.
fn write(world: &mut rusty_ecs::World, id: u32, op: impl FnOnce(&mut Volume)) {
    if let Some(mut v) = world.nav_modifier_mut(id) {
        op(&mut v);
    }
}

pub fn draw_nav_modifier(ui: &mut egui::Ui, world: &mut rusty_ecs::World, id: u32) {
    let Some(v) = world.nav_modifier(id).map(|v| v.clone()) else {
        return;
    };
    let mut remove = false;
    let title = "NavMesh Modifier Volume";
    component_card(ui, icon::SELECTION, title, Some(&mut remove), |ui| {
        let mut active = v.active;
        if ui.checkbox(&mut active, "Active").changed() {
            write(world, id, |v| volume_ops::set_active(v, active));
        }
        if let Some(c) = vec3_row(ui, "Center:", v.center) {
            write(world, id, |v| volume_ops::set_center(v, c));
        }
        if let Some(s) = vec3_row(ui, "Size:", v.size) {
            write(world, id, |v| volume_ops::set_size(v, s));
        }
        let mut area = v.area;
        let edited = ui
            .horizontal(|ui| {
                ui.label("Area:");
                let max = (MAX_AREAS - 1) as u8;
                ui.add(egui::DragValue::new(&mut area).ranged(0..=max))
                    .changed()
            })
            .inner;
        if edited {
            write(world, id, |v| volume_ops::set_area(v, i64::from(area)));
        }
        ui.label("Area ids are the scene's Navmesh area table (1: NotWalkable)");
    });
    if remove {
        remove_component(world, id, ComponentKind::NavMeshModifierVolume);
    }
}

impl crate::inspector::components::add::AddEntry
    for rusty_components::NavMeshModifierVolumeComponent
{
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Navigation;
    const LABEL: &'static str = "NavMesh Modifier Volume";
    const ICON: &'static str = egui_phosphor::regular::SELECTION;
}
