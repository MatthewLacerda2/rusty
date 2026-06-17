use crate::editor::EditorUi;
use std::fs;
use std::path::Path;
use std::time::Instant;

#[allow(clippy::too_many_lines)]
pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, path: &str) {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    ui.heading(format!("🎵 Audio: {}", filename));
    ui.add_space(5.0);

    // File metadata card
    egui::Frame::none()
        .fill(crate::editor::theme::from_ui(ui).bg_tier2)
        .inner_margin(8.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Path: {}", path));
                let size_str = if let Ok(meta) = fs::metadata(path) {
                    format_size(meta.len())
                } else {
                    "Unknown size".to_string()
                };
                ui.label(format!("Size: {}", size_str));
                ui.label("Type: Audio Clip");
            });
        });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    // Audio Import / Source settings
    ui.heading("Audio Clip Settings");
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.label("Volume:");
        ui.add(egui::Slider::new(&mut editor.asset_audio_volume, 0.0..=1.0));
    });

    ui.horizontal(|ui| {
        ui.label("Pitch:");
        ui.add(egui::Slider::new(&mut editor.asset_audio_pitch, 0.5..=2.0));
    });

    ui.checkbox(&mut editor.asset_audio_loop, "Loop Playback");
    ui.add_space(15.0);
    ui.separator();
    ui.add_space(5.0);

    // Live Playback Simulation
    ui.heading("Simulated Playback");
    ui.add_space(5.0);

    let total_duration = 30.0; // 30 seconds mock length

    // Calculate progress
    let mut progress = 0.0;
    if editor.asset_audio_playing {
        if let Some(start_time) = editor.asset_audio_play_time {
            let elapsed = start_time.elapsed().as_secs_f32();
            if elapsed >= total_duration {
                if editor.asset_audio_loop {
                    // Loop: reset start time to now
                    editor.asset_audio_play_time = Some(Instant::now());
                    progress = 0.0;
                } else {
                    // Stop playing
                    editor.asset_audio_playing = false;
                    editor.asset_audio_play_time = None;
                }
            } else {
                progress = elapsed / total_duration;
            }
        }
        // Force egui repaint to keep progress bar moving smoothly
        ui.ctx().request_repaint();
    }

    ui.horizontal(|ui| {
        if editor.asset_audio_playing {
            if ui.button("⏸ Pause").clicked() {
                editor.asset_audio_playing = false;
                editor.asset_audio_play_time = None;
            }
        } else {
            if ui.button("▶ Play").clicked() {
                editor.asset_audio_playing = true;
                editor.asset_audio_play_time = Some(Instant::now());
            }
        }

        if ui.button("⏹ Stop").clicked() {
            editor.asset_audio_playing = false;
            editor.asset_audio_play_time = None;
            progress = 0.0;
        }
    });

    ui.add_space(5.0);
    let current_sec = progress * total_duration;
    ui.label(format!(
        "Time: {:.1}s / {:.1}s",
        current_sec, total_duration
    ));
    ui.add(egui::ProgressBar::new(progress).show_percentage());
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
