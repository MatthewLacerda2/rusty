use egui_phosphor::regular as icon;
use glam::Mat4;

use crate::editor::inspector_card::component_card;
use crate::scene::Entity;

/// Transform editor with integrated parenting controls.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    parent_mat: Option<Mat4>,
    selected_parent_name: &str,
    valid_parents: &[(u32, String)],
    pending_parent_change: &mut Option<Option<u32>>,
    pending_nav_bake: &mut bool,
    is_dirty: &mut bool,
) {
    // Transform is mandatory: a foldout card with no remove button.
    component_card(ui, icon::ARROWS_OUT_CARDINAL, "Transform", None, |ui| {
        draw_body(
            ui,
            entity,
            parent_mat,
            selected_parent_name,
            valid_parents,
            pending_parent_change,
            pending_nav_bake,
            is_dirty,
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_body(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    parent_mat: Option<Mat4>,
    selected_parent_name: &str,
    valid_parents: &[(u32, String)],
    pending_parent_change: &mut Option<Option<u32>>,
    pending_nav_bake: &mut bool,
    is_dirty: &mut bool,
) {
    let is_static = entity.is_static;
    let trans = &mut entity.transform;

    let mut pos_changed = false;
    let mut rot_changed = false;
    let mut scl_changed = false;

    ui.label("Position:");
    ui.horizontal(|ui| {
        ui.label("X");
        pos_changed |= ui
            .add(egui::DragValue::new(&mut trans.position.x).speed(0.1))
            .changed();
        ui.label("Y");
        pos_changed |= ui
            .add(egui::DragValue::new(&mut trans.position.y).speed(0.1))
            .changed();
        ui.label("Z");
        pos_changed |= ui
            .add(egui::DragValue::new(&mut trans.position.z).speed(0.1))
            .changed();
    });

    ui.label("Rotation (Degrees):");
    let mut euler = trans.euler_angles();
    ui.horizontal(|ui| {
        ui.label("X");
        rot_changed |= ui
            .add(
                egui::DragValue::new(&mut euler.x)
                    .speed(1.0)
                    .clamp_range(-180.0..=180.0),
            )
            .changed();
        ui.label("Y");
        rot_changed |= ui
            .add(
                egui::DragValue::new(&mut euler.y)
                    .speed(1.0)
                    .clamp_range(-180.0..=180.0),
            )
            .changed();
        ui.label("Z");
        rot_changed |= ui
            .add(
                egui::DragValue::new(&mut euler.z)
                    .speed(1.0)
                    .clamp_range(-180.0..=180.0),
            )
            .changed();
    });
    if rot_changed {
        trans.set_euler_angles(euler);
    }

    ui.label("Scale:");
    ui.horizontal(|ui| {
        ui.label("X");
        scl_changed |= ui
            .add(
                egui::DragValue::new(&mut trans.scale.x)
                    .speed(0.05)
                    .clamp_range(0.01..=20.0),
            )
            .changed();
        ui.label("Y");
        scl_changed |= ui
            .add(
                egui::DragValue::new(&mut trans.scale.y)
                    .speed(0.05)
                    .clamp_range(0.01..=20.0),
            )
            .changed();
        ui.label("Z");
        scl_changed |= ui
            .add(
                egui::DragValue::new(&mut trans.scale.z)
                    .speed(0.05)
                    .clamp_range(0.01..=20.0),
            )
            .changed();
    });

    if pos_changed || rot_changed || scl_changed {
        entity.update_collider(parent_mat);
        *is_dirty = true;
        if is_static {
            *pending_nav_bake = true;
        }
    }

    // Integrated Parenting directly under Transform
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.label("Parent:");
        let mut current_sel = selected_parent_name.to_string();
        egui::ComboBox::from_id_source("ParentSelectionCombo")
            .selected_text(selected_parent_name.to_string())
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut current_sel, "None".to_string(), "None")
                    .clicked()
                {
                    *pending_parent_change = Some(None);
                }
                for (candidate_id, name) in valid_parents {
                    if ui
                        .selectable_value(&mut current_sel, name.clone(), name)
                        .clicked()
                    {
                        *pending_parent_change = Some(Some(*candidate_id));
                    }
                }
            });
    });
}
