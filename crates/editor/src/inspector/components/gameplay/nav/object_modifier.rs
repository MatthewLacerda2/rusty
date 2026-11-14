//! crates/editor/src/inspector/components/gameplay/nav/object_modifier.rs — the NavMesh
//! Modifier card (#1959).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::nav::object_modifier` ops `Component.Set` calls. A modifier
//! changes how the bake reads the object's colliders: the editor's per-frame nav
//! sync (#992) rebakes only those colliders' cells after an edit.

use crate::inspector::components::card::remove_component;
use crate::widgets::DragRange;
use egui_phosphor::regular as icon;
use rusty_scene::authoring::ComponentKind;

use crate::inspector::components::card::component_card;
use rusty_components::NavMeshModifierComponent as Modifier;
use rusty_navigation::{MAX_AREAS, NOT_WALKABLE_AREA};
use rusty_scene::authoring::nav::object_modifier as ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(
    world: &mut rusty_ecs::World,
    id: u32,
    is_dirty: &mut bool,
    op: impl FnOnce(&mut Modifier),
) {
    if let Some(mut m) = world.nav_mesh_modifier_mut(id) {
        op(&mut m);
    }
    *is_dirty = true;
}

/// 3EO. NavMeshModifier Component
pub fn draw_nav_mesh_modifier(
    ui: &mut egui::Ui,
    world: &mut rusty_ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(m) = world.nav_mesh_modifier(id).map(|m| m.clone()) else {
        return;
    };
    let d = is_dirty;
    let mut remove = false;
    component_card(
        ui,
        icon::FUNNEL,
        "NavMesh Modifier",
        Some(&mut remove),
        |ui| {
            let mut active = m.active;
            if ui.checkbox(&mut active, "Active").changed() {
                write(world, id, &mut *d, |m| ops::set_active(m, active));
            }
            let mut ignore = m.ignore_from_build;
            if ui.checkbox(&mut ignore, "Ignore From Build").changed() {
                write(world, id, &mut *d, |m| {
                    ops::set_ignore_from_build(m, ignore)
                });
            }
            let mut overrides = m.override_area.is_some();
            let mut area = m.override_area.unwrap_or(NOT_WALKABLE_AREA);
            let edited = ui
                .horizontal(|ui| {
                    let toggled = ui.checkbox(&mut overrides, "Override Area").changed();
                    let max = (MAX_AREAS - 1) as u8;
                    let drag = egui::DragValue::new(&mut area).ranged(0..=max);
                    toggled | ui.add_enabled(overrides, drag).changed()
                })
                .inner;
            if edited {
                let area = overrides.then_some(i64::from(area));
                write(world, id, &mut *d, |m| ops::set_override_area(m, area));
            }
            let mut children = m.apply_to_children;
            if ui.checkbox(&mut children, "Apply To Children").changed() {
                write(world, id, &mut *d, |m| {
                    ops::set_apply_to_children(m, children)
                });
            }
            ui.label("Furniture: Override Area 1 (NotWalkable). Ignore removes the object");
        },
    );
    if remove {
        remove_component(world, id, ComponentKind::NavMeshModifier, d);
    }
}

impl crate::inspector::components::add::AddEntry for rusty_components::NavMeshModifierComponent {
    const CATEGORY: crate::inspector::components::add::Category =
        crate::inspector::components::add::Category::Navigation;
    const LABEL: &'static str = "NavMesh Modifier";
    const ICON: &'static str = egui_phosphor::regular::FUNNEL;
}
