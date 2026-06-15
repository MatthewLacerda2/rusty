use glam::Vec3;

use egui_phosphor::regular as icon;

use crate::editor::{
    inspector_add, inspector_camera, inspector_gameplay, inspector_render, inspector_transform,
};
use crate::editor::{inspectors, EditorUi, InspectorTarget};
use crate::navigation::NavigationGraph;
use crate::scene::{Entity, Scene};
use crate::scripting::ConsoleLogs;

/// RIGHT PANEL: Properties Inspector
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &mut NavigationGraph,
) {
    let t = editor.theme;
    egui::SidePanel::right("Inspector Panel")
        .resizable(true)
        .width_range(260.0..=380.0)
        .frame(
            egui::Frame::none()
                .fill(t.bg_tier1)
                .inner_margin(t.space_md)
                .stroke(egui::Stroke::new(1.0, t.border)),
        )
        .show(ctx, |ui| {
            ui.heading(format!(
                "{}  Inspector",
                egui_phosphor::regular::SLIDERS_HORIZONTAL
            ));
            ui.separator();
            ui.add_space(5.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Dispatch on the explicit inspector target (mutually exclusive).
                match editor.inspector_target() {
                    InspectorTarget::Entity(selected_id) => {
                        draw_entity_inspector(editor, ui, scene, nav, selected_id);
                    }
                    InspectorTarget::Asset(asset_path) => {
                        inspectors::draw_inspector(ui, editor, scene, console, &asset_path);
                    }
                    InspectorTarget::SceneSettings => {
                        draw_global_settings(editor, ui, scene);
                    }
                }
            });
        });
}

fn draw_entity_inspector(
    editor: &mut EditorUi,
    ui: &mut egui::Ui,
    scene: &mut Scene,
    nav: &mut NavigationGraph,
    selected_id: u32,
) {
    let mut pending_parent_change = None;
    let mut pending_nav_bake = false;

    let current_parent_id = scene.get_entity(selected_id).and_then(|e| e.parent_id);
    let parent_mat = current_parent_id.map(|p| scene.compute_world_matrix(p));
    let selected_parent_name = if let Some(p_id) = current_parent_id {
        scene
            .get_entity(p_id)
            .map(|e| e.name.clone())
            .unwrap_or("None".to_string())
    } else {
        "None".to_string()
    };

    let candidates: Vec<(u32, String)> = scene
        .iter()
        .filter(|e| e.id != selected_id)
        .map(|e| (e.id, e.name.clone()))
        .collect();
    let valid_parents: Vec<(u32, String)> = candidates
        .into_iter()
        .filter(|(id, _)| {
            let mut curr = *id;
            while let Some(ancestor) = scene.get_entity(curr).and_then(|x| x.parent_id) {
                if ancestor == selected_id {
                    return false;
                }
                curr = ancestor;
            }
            true
        })
        .collect();

    if let Some(mut entity_guard) = scene.get_entity_mut(selected_id) {
        let entity: &mut Entity = &mut entity_guard;
        if entity.camera.is_none() && entity.visual_correction.is_some() {
            entity.visual_correction = None;
        }

        // Unity-style object header: name + active/static toggles, on a raised card.
        let t = editor.theme;
        egui::Frame::none()
            .fill(t.bg_tier2)
            .inner_margin(t.space_sm)
            .rounding(4.0)
            .stroke(egui::Stroke::new(1.0, t.border))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(t.text_secondary, icon::CUBE_FOCUS);
                    ui.add(
                        egui::TextEdit::singleline(&mut entity.name).desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut entity.active, "Active");
                    if ui
                        .checkbox(&mut entity.is_static, "Static")
                        .on_hover_text("Blocks the navmesh")
                        .changed()
                    {
                        pending_nav_bake = true;
                    }
                });
            });
        ui.add_space(t.space_xs);

        inspector_transform::draw(
            ui,
            entity,
            parent_mat,
            &selected_parent_name,
            &valid_parents,
            &mut pending_parent_change,
            &mut pending_nav_bake,
            &mut editor.is_dirty,
        );

        inspector_render::draw_mesh(ui, entity, &mut editor.is_dirty);
        inspector_render::draw_texture(ui, entity, &mut editor.is_dirty);
        inspector_render::draw_light(ui, entity, &mut editor.is_dirty);

        inspector_gameplay::draw_script(ui, entity, &mut editor.is_dirty);
        inspector_gameplay::draw_health(ui, entity, &mut editor.is_dirty);
        inspector_gameplay::draw_collider(ui, entity, &mut editor.is_dirty, &mut pending_nav_bake);
        inspector_gameplay::draw_rigidbody(ui, entity, &mut editor.is_dirty);
        inspector_gameplay::draw_nav_agent(ui, entity, &mut editor.is_dirty);

        inspector_camera::draw_camera(ui, entity, &mut editor.is_dirty);
        inspector_camera::draw_visual_correction(ui, entity, &mut editor.is_dirty);

        inspector_add::draw(ui, entity);
    }

    if let Some(new_parent) = pending_parent_change {
        let _ = scene.set_parent(selected_id, new_parent);
    }
    if pending_nav_bake {
        nav.bake(scene);
    }
}

fn draw_global_settings(editor: &mut EditorUi, ui: &mut egui::Ui, scene: &mut Scene) {
    let t = crate::editor::theme::from_ui(ui);
    ui.heading(format!("{}  Scene Settings", egui_phosphor::regular::GLOBE));
    ui.separator();
    ui.add_space(5.0);

    // 1. Skybox Path
    ui.horizontal(|ui| {
        ui.label("Skybox:");
        let response = ui.text_edit_singleline(&mut scene.skybox_path);
        if response.changed() {
            editor.is_dirty = true;
        }
    });
    ui.colored_label(
        t.text_secondary,
        "Provide a path to a panoramic image\n(e.g. assets/textures/sky.png)",
    );
    ui.add_space(8.0);

    // 2. Ambient Light Color
    ui.label("Ambient Color:");
    ui.horizontal(|ui| {
        let mut color_arr = [
            scene.ambient_color.x,
            scene.ambient_color.y,
            scene.ambient_color.z,
        ];
        if ui.color_edit_button_rgb(&mut color_arr).changed() {
            scene.ambient_color = Vec3::new(color_arr[0], color_arr[1], color_arr[2]);
            editor.is_dirty = true;
        }
    });
    ui.add_space(8.0);

    // 3. Ambient Light Intensity
    ui.label("Ambient Intensity:");
    let response =
        ui.add(egui::Slider::new(&mut scene.ambient_intensity, 0.0..=5.0).text("intensity"));
    if response.changed() {
        editor.is_dirty = true;
    }

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);
    ui.colored_label(
        t.text_secondary,
        "Select an entity from Hierarchy\nor click a project asset to inspect.",
    );
}
