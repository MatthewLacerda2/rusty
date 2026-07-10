use egui_phosphor::regular as icon;
use glam::Mat4;

use crate::editor::inspector::components::card::component_card;

/// Transform editor with integrated parenting controls.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
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
            world,
            id,
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
    world: &mut crate::ecs::World,
    id: u32,
    parent_mat: Option<Mat4>,
    selected_parent_name: &str,
    valid_parents: &[(u32, String)],
    pending_parent_change: &mut Option<Option<u32>>,
    pending_nav_bake: &mut bool,
    is_dirty: &mut bool,
) {
    let is_static = world.is_static(id);

    let Some(mut trans) = world.transform_mut(id) else {
        return;
    };
    let pos_changed = draw_position(ui, &mut trans);
    let rot_changed = draw_rotation(ui, &mut trans);
    let scl_changed = draw_scale(ui, &mut trans);
    drop(trans);

    if pos_changed || rot_changed || scl_changed {
        let local = world.transform(id).map(|t| t.to_matrix());
        if let Some(local) = local {
            let wm = match parent_mat {
                Some(p) => p * local,
                None => local,
            };
            if let Some(mut col) = world.collider_mut(id) {
                let (min, max) = col.calculate_world_aabb(wm);
                col.aabb_min = min;
                col.aabb_max = max;
            }
        }
        *is_dirty = true;
        if is_static {
            *pending_nav_bake = true;
        }
    }

    draw_parent_selector(
        ui,
        selected_parent_name,
        valid_parents,
        pending_parent_change,
    );
}

/// The Position row (x/y/z drag fields). Returns whether any axis changed.
fn draw_position(ui: &mut egui::Ui, trans: &mut crate::components::TransformComponent) -> bool {
    ui.label("Position:");
    ui.horizontal(|ui| {
        ui.label("X");
        let mut c = ui
            .add(egui::DragValue::new(&mut trans.position.x).speed(0.1))
            .changed();
        ui.label("Y");
        c |= ui
            .add(egui::DragValue::new(&mut trans.position.y).speed(0.1))
            .changed();
        ui.label("Z");
        c |= ui
            .add(egui::DragValue::new(&mut trans.position.z).speed(0.1))
            .changed();
        c
    })
    .inner
}

/// The Rotation row in degrees. Writes back the euler angles when changed and
/// returns whether any axis changed.
fn draw_rotation(ui: &mut egui::Ui, trans: &mut crate::components::TransformComponent) -> bool {
    ui.label("Rotation (Degrees):");
    let mut euler = trans.euler_angles();
    let rot_changed = ui
        .horizontal(|ui| {
            ui.label("X");
            let mut c = drag_angle(ui, &mut euler.x);
            ui.label("Y");
            c |= drag_angle(ui, &mut euler.y);
            ui.label("Z");
            c |= drag_angle(ui, &mut euler.z);
            c
        })
        .inner;
    if rot_changed {
        trans.set_euler_angles(euler);
    }
    rot_changed
}

/// A single degree drag field clamped to [-180, 180]. Returns whether it changed.
fn drag_angle(ui: &mut egui::Ui, value: &mut f32) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .speed(1.0)
            .clamp_range(-180.0..=180.0),
    )
    .changed()
}

/// The Scale row (x/y/z drag fields). Returns whether any axis changed.
fn draw_scale(ui: &mut egui::Ui, trans: &mut crate::components::TransformComponent) -> bool {
    ui.label("Scale:");
    ui.horizontal(|ui| {
        ui.label("X");
        let mut c = drag_scale(ui, &mut trans.scale.x);
        ui.label("Y");
        c |= drag_scale(ui, &mut trans.scale.y);
        ui.label("Z");
        c |= drag_scale(ui, &mut trans.scale.z);
        c
    })
    .inner
}

/// A single scale drag field clamped to [0.01, 20]. Returns whether it changed.
fn drag_scale(ui: &mut egui::Ui, value: &mut f32) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .speed(0.05)
            .clamp_range(0.01..=20.0),
    )
    .changed()
}

/// The integrated parent-selection combo box directly under Transform.
fn draw_parent_selector(
    ui: &mut egui::Ui,
    selected_parent_name: &str,
    valid_parents: &[(u32, String)],
    pending_parent_change: &mut Option<Option<u32>>,
) {
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
