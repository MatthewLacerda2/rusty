//! The Selectable inspector card (#420): interactable, the transition with its
//! target graphic and per-state colours / sprites, and navigation with its explicit
//! targets. Widgets edit a snapshot; every write routes through
//! `scene::authoring::selectable`.

use egui_phosphor::regular as icon;

use super::combo;
use crate::components::{
    NavigationMode, SelectableComponent, SelectableTransition, SelectionState,
};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::selectable as ops;

/// Selectable card. A THIN client (#287): widgets edit a snapshot and the changed
/// snapshot is written back through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.selectable(id).map(|s| s.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::CURSOR_CLICK,
        "Selectable",
        Some(&mut remove),
        |ui| {
            changed |= ui
                .checkbox(&mut edit.interactable, "Interactable")
                .changed();
            changed |= draw_transition(ui, &mut edit);
            changed |= draw_navigation(ui, &mut edit);
        },
    );
    if changed {
        if let Some(mut s) = world.selectable_mut(id) {
            write_back(&mut s, &edit);
        }
    }
    if remove {
        world.set_selectable(id, None);
    }
    *is_dirty |= remove || changed;
}

/// Transition, target graphic, and the per-state colours + fade or sprites.
fn draw_transition(ui: &mut egui::Ui, edit: &mut SelectableComponent) -> bool {
    let kinds = [
        SelectableTransition::None,
        SelectableTransition::ColorTint,
        SelectableTransition::SpriteSwap,
    ];
    let mut changed = combo(
        ui,
        "Transition",
        &mut edit.transition,
        &kinds,
        ops::transition_name,
    );
    changed |= entity_row(ui, "Target Graphic:", "self", &mut edit.target_graphic);
    let states = &SelectionState::ALL;
    match edit.transition {
        SelectableTransition::ColorTint => {
            for (c, s) in edit.colors.iter_mut().zip(states) {
                let mut rgba = c.to_array();
                changed |= ui
                    .horizontal(|ui| {
                        ui.label(format!("{}:", s.name()));
                        ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed()
                    })
                    .inner;
                *c = glam::Vec4::from_array(rgba);
            }
            changed |= ui
                .horizontal(|ui| {
                    ui.label("Fade Duration:");
                    let drag = egui::DragValue::new(&mut edit.fade_duration).speed(0.01);
                    ui.add(drag.clamp_range(0.0..=f32::MAX).suffix(" s"))
                        .changed()
                })
                .inner;
        }
        SelectableTransition::SpriteSwap => {
            for (slot, s) in edit.sprites.iter_mut().zip(states).skip(1) {
                let mut path = slot.clone().unwrap_or_default();
                changed |= ui
                    .horizontal(|ui| {
                        ui.label(format!("{} Sprite:", s.name()));
                        ui.text_edit_singleline(&mut path).changed()
                    })
                    .inner;
                *slot = Some(path).filter(|p| !p.is_empty());
            }
        }
        SelectableTransition::None => {}
    }
    changed
}

/// Navigation mode and, for `Explicit`, the four targets.
fn draw_navigation(ui: &mut egui::Ui, edit: &mut SelectableComponent) -> bool {
    let modes = [
        NavigationMode::None,
        NavigationMode::Automatic,
        NavigationMode::Explicit,
    ];
    let mut changed = combo(
        ui,
        "Navigation",
        &mut edit.navigation,
        &modes,
        ops::navigation_name,
    );
    if edit.navigation == NavigationMode::Explicit {
        for (target, dir) in edit.select_on.iter_mut().zip(ops::DIRECTIONS) {
            changed |= entity_row(ui, &format!("Select On {dir}:"), "none", target);
        }
    }
    changed
}

/// An entity-id field: `0` reads and writes as `None` (shown as `empty`).
fn entity_row(ui: &mut egui::Ui, label: &str, empty: &str, value: &mut Option<u32>) -> bool {
    let mut raw = value.unwrap_or(0);
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let drag = egui::DragValue::new(&mut raw).custom_formatter(|v, _| {
                if v == 0.0 {
                    empty.to_string()
                } else {
                    format!("#{v}")
                }
            });
            ui.add(drag).changed()
        })
        .inner;
    *value = (raw != 0).then_some(raw);
    changed
}

/// Write the edited snapshot back through the shared ops.
fn write_back(s: &mut SelectableComponent, edit: &SelectableComponent) {
    ops::set_interactable(s, edit.interactable);
    ops::set_transition(s, edit.transition);
    ops::set_target_graphic(s, edit.target_graphic);
    for state in SelectionState::ALL {
        ops::set_color(s, state, edit.color(state));
        ops::set_sprite(s, state, edit.sprites[state as usize].clone());
    }
    ops::set_fade_duration(s, edit.fade_duration);
    ops::set_navigation(s, edit.navigation);
    for (dir, target) in edit.select_on.iter().enumerate() {
        ops::set_select_on(s, dir, *target);
    }
}
