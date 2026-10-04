//! src/editor/inspector/components/gameplay/nav_modifier.rs — the NavMesh Modifier
//! Volume card (#460).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::nav_modifier` ops the `NavMeshModifierVolume.*` Lua setters
//! call. A volume assigns areas at bake time, so every edit asks for a nav rebake
//! (incremental: only the volume's cells).

use egui_phosphor::regular as icon;

use super::physics::vec3_row;
use crate::components::NavMeshModifierVolumeComponent as Volume;
use crate::editor::inspector::components::card::component_card;
use crate::navigation::MAX_AREAS;
use crate::scene::authoring::nav_modifier as volume_ops;

/// Route one edit through the shared ops, mark the scene dirty, ask for a rebake.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    flags: (&mut bool, &mut bool),
    op: impl FnOnce(&mut Volume),
) {
    if let Some(mut v) = world.nav_modifier_mut(id) {
        op(&mut v);
    }
    *flags.0 = true;
    *flags.1 = true;
}

/// 3EN. NavMeshModifierVolume Component
pub fn draw_nav_modifier(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    let Some(v) = world.nav_modifier(id).map(|v| v.clone()) else {
        return;
    };
    let (d, b) = (is_dirty, pending_nav_bake);
    let mut remove = false;
    let title = "NavMesh Modifier Volume";
    component_card(ui, icon::SELECTION, title, Some(&mut remove), |ui| {
        let mut active = v.active;
        if ui.checkbox(&mut active, "Active").changed() {
            write(world, id, (&mut *d, &mut *b), |v| {
                volume_ops::set_active(v, active)
            });
        }
        if let Some(c) = vec3_row(ui, "Center:", v.center) {
            write(world, id, (&mut *d, &mut *b), |v| {
                volume_ops::set_center(v, c)
            });
        }
        if let Some(s) = vec3_row(ui, "Size:", v.size) {
            write(world, id, (&mut *d, &mut *b), |v| {
                volume_ops::set_size(v, s)
            });
        }
        let mut area = v.area;
        let edited = ui
            .horizontal(|ui| {
                ui.label("Area:");
                let max = (MAX_AREAS - 1) as u8;
                ui.add(egui::DragValue::new(&mut area).range(0..=max))
                    .changed()
            })
            .inner;
        if edited {
            write(world, id, (&mut *d, &mut *b), |v| {
                volume_ops::set_area(v, i64::from(area))
            });
        }
        ui.label("Area ids are the scene's Navmesh area table (1: NotWalkable)");
    });
    if remove {
        world.set_nav_modifier(id, None);
        *d = true;
        *b = true;
    }
}
