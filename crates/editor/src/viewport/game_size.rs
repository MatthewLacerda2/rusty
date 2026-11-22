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

pub fn fit(
    setting: GameViewSetting,
    panel: egui::Vec2,
    pixels_per_point: f32,
) -> Option<(u32, u32)> {
    let px = |v: f32| (v * pixels_per_point).round().max(0.0) as u32;
    setting.resolve((px(panel.x), px(panel.y)), pixels_per_point)
}

pub fn letterbox(panel: egui::Vec2, size: (u32, u32)) -> egui::Vec2 {
    let (_, (w, h)) = rusty_core::application::letterbox((panel.x, panel.y), size);
    egui::vec2(w, h)
}

/// The renderer's `screen_box`: where the Scene tab's UI overlay draws, `(x, y, width,
/// height)` in the image's `panel` pixels, the box `OverlayFrame` maps clicks through.
pub fn overlay_box(
    interaction: &super::ViewportInteraction,
    panel: (u32, u32),
) -> Option<(u32, u32, u32, u32)> {
    let scene_tab = interaction.tab == super::ViewportTab::Scene;
    let game = interaction.screen.filter(|_| scene_tab)?;
    let panel = (panel.0 as f32, panel.1 as f32);
    let ((x, y), (w, h)) = rusty_core::application::letterbox(panel, game);
    let px = |v: f32| v.round().max(0.0) as u32;
    Some((px(x), px(y), px(w).max(1), px(h).max(1)))
}

/// The Scene tab's [`OverlayFrame`](super::rect_tool::OverlayFrame) for the image
/// `interaction` reports, at `pixels_per_point`, boxed at the Game view's own size.
pub fn overlay_frame(
    interaction: &super::ViewportInteraction,
    pixels_per_point: f32,
    textures: &std::rc::Rc<rusty_ui::TextureSizes>,
) -> super::rect_tool::OverlayFrame {
    let size = glam::Vec2::new(interaction.size.x, interaction.size.y);
    super::rect_tool::OverlayFrame::new(size, pixels_per_point, interaction.screen, textures)
}
