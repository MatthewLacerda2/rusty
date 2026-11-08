//! The **Generate Lighting** card's settings (#810, #832): what the bake reads from
//! the scene's lighting settings, as `Lighting.SetSettings{…}` edits them.

use crate::editor::widgets::DragRange;
use crate::scene::lighting::lightmap::{BakeSettings, Denoiser};
use crate::scene::lighting::{DiffuseModel, MixedLightingMode};

/// The scene's lightmap knobs; returns whether any changed.
pub(super) fn draw_settings(ui: &mut egui::Ui, s: &mut BakeSettings) -> bool {
    let mut changed = false;
    let mut row = |ui: &mut egui::Ui, drag: egui::DragValue, label: &str, hover: &str| {
        ui.horizontal(|ui| {
            changed |= ui.add(drag).on_hover_text(hover).changed();
            ui.label(label);
        });
    };
    let texels = egui::DragValue::new(&mut s.texels_per_unit).ranged(BakeSettings::TEXELS_PER_UNIT);
    row(
        ui,
        texels.speed(0.1),
        "Texels per unit",
        "Lightmap resolution",
    );
    let samples = egui::DragValue::new(&mut s.samples).ranged(BakeSettings::SAMPLES);
    row(
        ui,
        samples,
        "Samples",
        "Paths traced per texel; more is smoother and slower",
    );
    let bounces = egui::DragValue::new(&mut s.bounces).ranged(BakeSettings::BOUNCES);
    row(
        ui,
        bounces,
        "Bounces",
        "Surfaces a path bounces off before it stops",
    );
    changed |= draw_max_lightmap_size(ui, &mut s.max_lightmap_size);
    changed |= draw_denoiser(ui, &mut s.denoiser);
    changed |= ui
        .checkbox(&mut s.directional, "Directional lightmaps")
        .on_hover_text("Bake light directions too, so normal maps show under baked light (#810)")
        .changed();
    changed
}

/// Unity's lightmap *Denoiser* (#1884); returns whether it changed.
fn draw_denoiser(ui: &mut egui::Ui, denoiser: &mut Denoiser) -> bool {
    let before = *denoiser;
    egui::ComboBox::from_label("Denoiser")
        .selected_text(denoiser.name())
        .show_ui(ui, |ui| {
            for d in Denoiser::ALL {
                ui.selectable_value(denoiser, d, d.name());
            }
        })
        .response
        .on_hover_text(
            "OIDN cleans the bounce from few samples (needs Open Image Denoise); \
             Filter blurs it, edge-aware; None leaves it raw",
        );
    *denoiser != before
}

/// Unity's *Max Lightmap Size* (#1704): the atlas page edge, a power of two from
/// [`BakeSettings::MAX_LIGHTMAP_SIZE`]; returns whether it changed.
fn draw_max_lightmap_size(ui: &mut egui::Ui, size: &mut u32) -> bool {
    let before = *size;
    let (low, high) = BakeSettings::MAX_LIGHTMAP_SIZE.into_inner();
    let edges = std::iter::successors(Some(low), |e| Some(e * 2)).take_while(|&e| e <= high);
    egui::ComboBox::from_label("Max Lightmap Size")
        .selected_text(size.to_string())
        .show_ui(ui, |ui| {
            for edge in edges {
                ui.selectable_value(size, edge, edge.to_string());
            }
        })
        .response
        .on_hover_text("Atlas page edge in texels; the largest lightmap a mesh can get");
    *size != before
}

/// The scene's diffuse model (#1698); returns whether it changed. Live lights follow
/// it at once, baked light from the next Generate Lighting.
pub(super) fn draw_diffuse_model(ui: &mut egui::Ui, model: &mut DiffuseModel) -> bool {
    let before = *model;
    egui::ComboBox::from_label("Diffuse model")
        .selected_text(model.name())
        .show_ui(ui, |ui| {
            for m in DiffuseModel::ALL {
                ui.selectable_value(model, m, m.name());
            }
        })
        .response
        .on_hover_text(
            "Lambert, or the retro half-Lambert wrap; Realtime and baked lamps both use it",
        );
    *model != before
}

/// The scene's Mixed Lighting Mode (#1695); returns whether it changed. It takes
/// effect at the next Generate Lighting.
pub(super) fn draw_mixed_mode(ui: &mut egui::Ui, mode: &mut MixedLightingMode) -> bool {
    let before = *mode;
    egui::ComboBox::from_label("Mixed lighting")
        .selected_text(mode.name())
        .show_ui(ui, |ui| {
            for m in MixedLightingMode::ALL {
                ui.selectable_value(mode, m, m.name());
            }
        })
        .response
        .on_hover_text("Shadowmask also bakes Mixed lights' static shadows (next bake)");
    *mode != before
}
