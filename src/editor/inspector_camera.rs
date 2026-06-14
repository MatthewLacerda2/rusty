use crate::core::scene::{Entity, Tonemap};

/// Camera Component panel
pub fn draw_camera(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_camera = false;
    if let Some(cam) = &mut entity.camera {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("🎥 Camera Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Camera").clicked() {
                    remove_camera = true;
                }
            });
        });

        // Force high-quality motion blur by default under the hood
        cam.motion_blur_active = true;
        cam.motion_blur_samples = 64;

        ui.horizontal(|ui| {
            ui.label("FOV:");
            ui.add(egui::Slider::new(&mut cam.fov, 1.0..=120.0));
        });

        ui.horizontal(|ui| {
            ui.label("Near Clip:");
            ui.add(
                egui::DragValue::new(&mut cam.near)
                    .speed(0.01)
                    .clamp_range(0.01..=10.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Far Clip:");
            ui.add(
                egui::DragValue::new(&mut cam.far)
                    .speed(1.0)
                    .clamp_range(1.0..=1000.0),
            );
        });

        ui.colored_label(
            egui::Color32::from_rgb(0, 242, 254),
            "✔ Intrinsic Motion Blur (Active | 64 Samples)",
        );
    }
    if remove_camera {
        entity.camera = None;
        entity.visual_correction = None;
        *is_dirty = true;
    }
}

/// Visual Correction Component panel
pub fn draw_visual_correction(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_vc = false;
    if let Some(vc) = &mut entity.visual_correction {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("🎨 Visual Correction Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("🗑")
                    .on_hover_text("Remove Visual Correction")
                    .clicked()
                {
                    remove_vc = true;
                }
            });
        });

        ui.checkbox(&mut vc.active, "Enable Visual Correction");

        ui.add_space(3.0);
        ui.label("✨ Bloom");
        ui.checkbox(&mut vc.bloom_active, "  Bloom Active");
        if vc.bloom_active {
            ui.horizontal(|ui| {
                ui.label("    Intensity:");
                ui.add(egui::Slider::new(&mut vc.bloom_intensity, 0.0..=5.0));
            });
            ui.horizontal(|ui| {
                ui.label("    Threshold:");
                ui.add(egui::Slider::new(&mut vc.bloom_threshold, 0.0..=1.0));
            });
        }

        ui.add_space(3.0);
        ui.label("🌈 Color Correction");
        ui.horizontal(|ui| {
            ui.label("  Exposure (EV):");
            ui.add(egui::Slider::new(&mut vc.exposure, -4.0..=4.0));
        });
        ui.horizontal(|ui| {
            ui.label("  Contrast:");
            ui.add(egui::Slider::new(&mut vc.contrast, 0.0..=2.0));
        });
        ui.horizontal(|ui| {
            ui.label("  Saturation:");
            ui.add(egui::Slider::new(&mut vc.saturation, 0.0..=2.0));
        });
        ui.horizontal(|ui| {
            ui.label("  Tonemap:");
            egui::ComboBox::from_id_source("tonemap")
                .selected_text(format!("{:?}", vc.tonemap))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut vc.tonemap, Tonemap::Aces, "ACES");
                    ui.selectable_value(&mut vc.tonemap, Tonemap::Reinhard, "Reinhard");
                    ui.selectable_value(&mut vc.tonemap, Tonemap::None, "None");
                });
        });
        ui.horizontal(|ui| {
            ui.label("  Gamma:");
            ui.add(egui::Slider::new(&mut vc.gamma, 1.0..=3.0));
        });

        ui.add_space(3.0);
        ui.label("🪞 Screen Space Reflections (SSR)");
        ui.checkbox(&mut vc.ssr_active, "  SSR Active");
        if vc.ssr_active {
            ui.horizontal(|ui| {
                ui.label("    Quality:");
                egui::ComboBox::from_label("")
                    .selected_text(&vc.ssr_quality)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut vc.ssr_quality, "Low".to_string(), "Low");
                        ui.selectable_value(&mut vc.ssr_quality, "Medium".to_string(), "Medium");
                        ui.selectable_value(&mut vc.ssr_quality, "High".to_string(), "High");
                        ui.selectable_value(&mut vc.ssr_quality, "Ultra".to_string(), "Ultra");
                    });
            });
            ui.checkbox(&mut vc.ssr_temporal_upsampling, "    Temporal Upsampling");
        }
    }
    if remove_vc {
        entity.visual_correction = None;
        *is_dirty = true;
    }
}
