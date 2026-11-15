//! The Game view's size dropdown (#2207), Unity's Game view resolution menu: *Free
//! Aspect*, 16:9, 16:10, 4:3, 1920x1080, 1280x720, a custom size a script set
//! (`Editor.SetGameViewSize`), and "1x on HiDPI" (Unity's *Low Resolution Aspect
//! Ratios*). The setting and how it resolves are `core::application::game_view`'s;
//! this is the control on the Game tab's strip and the size it gives the image.

use rusty_core::application::{GameViewSetting, GameViewSize};

/// The dropdown on the Game tab's strip. `enabled` is false while a demo replays
/// on the size it recorded: the setting waits until it closes.
pub fn dropdown(ui: &mut egui::Ui, setting: &mut GameViewSetting, enabled: bool) {
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt("game_view_size")
            .selected_text(setting.size.label())
            .show_ui(ui, |ui| {
                let presets = GameViewSize::PRESETS.iter().map(|(size, _)| *size);
                let custom = Some(setting.size).filter(|s| !presets.clone().any(|p| p == *s));
                for size in presets.chain(custom) {
                    ui.selectable_value(&mut setting.size, size, size.label());
                }
                ui.separator();
                ui.checkbox(&mut setting.low_resolution, "1x on HiDPI")
                    .on_hover_text(
                        "Size Free Aspect and the aspects by the panel's points, not its \
                         pixels: a Retina panel shows the game at 1x",
                    );
            })
            .response
            .on_hover_text("The Game view's resolution: what the game renders and lays out at");
    });
}

/// The pixel size the Game view renders at in a panel of `panel` points at
/// `pixels_per_point`; `None` when it is the panel's own (drawn unboxed, as before).
pub fn fit(
    setting: GameViewSetting,
    panel: egui::Vec2,
    pixels_per_point: f32,
) -> Option<(u32, u32)> {
    let px = |v: f32| (v * pixels_per_point).round().max(0.0) as u32;
    setting.resolve((px(panel.x), px(panel.y)), pixels_per_point)
}

/// The largest size of `width:height`'s aspect ratio that fits in `panel`.
pub fn letterbox(panel: egui::Vec2, (width, height): (u32, u32)) -> egui::Vec2 {
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    if panel.x / panel.y.max(f32::MIN_POSITIVE) > aspect {
        egui::vec2(panel.y * aspect, panel.y)
    } else {
        egui::vec2(panel.x, panel.x / aspect)
    }
}
