use crate::core::scene::Scene;
use crate::editor::EditorUi;
use crate::scripting::ConsoleLogs;

/// TOP HEADER PANEL (Controls engine state) — ALWAYS VISIBLE
pub fn draw(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: &mut bool,
) {
    egui::TopBottomPanel::top("Header Panel")
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(22, 22, 30))
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(30, 30, 40))),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let purple_bg = egui::Color32::from_rgb(124, 77, 255); // Premium indigo

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let play_btn = if *is_playing {
                        egui::Button::new(
                            egui::RichText::new("▶ Play")
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(purple_bg)
                    } else {
                        egui::Button::new(egui::RichText::new("▶ Play").color(egui::Color32::GRAY))
                    };
                    if ui.add(play_btn).clicked() {
                        *is_playing = true;
                    }

                    let stop_btn = if !*is_playing {
                        egui::Button::new(
                            egui::RichText::new("■ Stop")
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(purple_bg)
                    } else {
                        egui::Button::new(egui::RichText::new("■ Stop").color(egui::Color32::GRAY))
                    };
                    if ui.add(stop_btn).clicked() {
                        *is_playing = false;
                        scene.selected_entity_id = None;
                        editor.selected_entity_id = None;
                    }

                    ui.separator();
                    ui.label(format!(
                        "Mode: {}",
                        if *is_playing {
                            "🎮 PLAYMODE"
                        } else {
                            "🛠️ EDITORMODE"
                        }
                    ));

                    // Quick scene save/load buttons in header
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("💾 Save Scene").clicked() {
                            if let Err(err) = scene.save_to_file("project/scenes/demo.scene") {
                                console.error(format!("Failed to save scene: {}", err));
                            } else {
                                console.info(
                                    "Scene saved successfully to project/scenes/demo.scene"
                                        .to_string(),
                                );
                            }
                        }
                        if ui.button("📂 Load Scene").clicked() {
                            if let Err(err) = scene.load_from_file("project/scenes/demo.scene") {
                                console.error(format!("Failed to load scene: {}", err));
                            } else {
                                console.info(
                                    "Scene loaded successfully from project/scenes/demo.scene"
                                        .to_string(),
                                );
                            }
                        }
                    });
                });
            });
        });
}
