//! The **Generate Lighting** card's settings: what the bake reads from
//! the scene's lighting settings, as `Lighting.SetSettings{…}` edits them.

use crate::widgets::DragRange;
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};
use rusty_scene::lighting::{DiffuseModel, MixedLightingMode};

/// The scene's lightmap knobs.
pub(super) fn draw_settings(ui: &mut egui::Ui, s: &mut BakeSettings) {
    let texels = egui::DragValue::new(&mut s.texels_per_unit).ranged(BakeSettings::TEXELS_PER_UNIT);
    row(
        ui,
        texels.speed(0.1),
        "Texels per unit",
        "Lightmap resolution",
    );
    draw_samples(ui, s);
    let bounces = egui::DragValue::new(&mut s.bounces).ranged(BakeSettings::BOUNCES);
    row(
        ui,
        bounces,
        "Bounces",
        "Surfaces a path bounces off before it stops",
    );
    let tolerance = egui::DragValue::new(&mut s.backface_tolerance)
        .ranged(BakeSettings::BACKFACE_TOLERANCE)
        .speed(0.01);
    row(
        ui,
        tolerance,
        "Backface tolerance",
        "Share of a texel's rays that must see front faces; below it the texel is \
         inside geometry and its own chart fills it",
    );
    draw_max_lightmap_size(ui, &mut s.max_lightmap_size);
    draw_denoiser(ui, &mut s.denoiser);
    ui.checkbox(&mut s.directional, "Directional lightmaps")
        .on_hover_text("Bake light directions too, so normal maps show under baked light (#810)");
    draw_progressive(ui, s);
}

/// The paths per texel and Unity's *Direct Samples* beside them.
fn draw_samples(ui: &mut egui::Ui, s: &mut BakeSettings) {
    let samples = egui::DragValue::new(&mut s.samples).ranged(BakeSettings::SAMPLES);
    row(
        ui,
        samples,
        "Samples",
        "Paths traced per texel; more is smoother and slower",
    );
    let direct = egui::DragValue::new(&mut s.direct_samples)
        .ranged(BakeSettings::DIRECT_SAMPLES)
        .custom_formatter(|n, _| match n as u32 {
            0 => "as Samples".into(),
            n => n.to_string(),
        });
    row(
        ui,
        direct,
        "Direct samples",
        "Points each Area light's direct light is integrated over; 0 follows Samples. \
         From 50 up a probe prices a fully lit or hidden panel at a fixed 24 or 52 rays, \
         so this only prices penumbrae; under 50 every panel takes every point, so 49 \
         costs more than 64 and only about 25 or under is cheaper",
    );
}

/// One drag value labelled `label`, `hover` on it.
fn row(ui: &mut egui::Ui, drag: egui::DragValue, label: &str, hover: &str) {
    ui.horizontal(|ui| {
        ui.add(drag).on_hover_text(hover);
        ui.label(label);
    });
}

/// The progressive bake's knobs.
fn draw_progressive(ui: &mut egui::Ui, s: &mut BakeSettings) {
    let per_pass = egui::DragValue::new(&mut s.samples_per_pass).ranged(BakeSettings::SAMPLES);
    row(
        ui,
        per_pass,
        "Samples per pass",
        "Paths each pass adds to every texel; the scene shows the lightmaps after each pass",
    );
    ui.checkbox(&mut s.prioritize_view, "Prioritize view")
        .on_hover_text(
            "After the whole scene's first pass, what the Scene view's camera sees gets \
             all its passes first, then the rest gets theirs",
        );
}

/// Unity's lightmap *Denoiser*.
fn draw_denoiser(ui: &mut egui::Ui, denoiser: &mut Denoiser) {
    egui::ComboBox::from_label("Denoiser")
        .selected_text(denoiser.name())
        .show_ui(ui, |ui| {
            for d in Denoiser::ALL {
                ui.selectable_value(denoiser, d, d.name());
            }
        })
        .response
        .on_hover_text(
            "OIDN cleans the bounce from few samples, then Filter smooths it (needs \
             Open Image Denoise); Filter alone blurs it, edge-aware; None leaves it raw",
        );
}

fn draw_max_lightmap_size(ui: &mut egui::Ui, size: &mut u32) {
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
}

pub(super) fn draw_diffuse_model(ui: &mut egui::Ui, model: &mut DiffuseModel) {
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
}

/// The scene's Mixed Lighting Mode. It takes effect at the next Generate Lighting.
pub(super) fn draw_mixed_mode(ui: &mut egui::Ui, mode: &mut MixedLightingMode) {
    egui::ComboBox::from_label("Mixed lighting")
        .selected_text(mode.name())
        .show_ui(ui, |ui| {
            for m in MixedLightingMode::ALL {
                ui.selectable_value(mode, m, m.name());
            }
        })
        .response
        .on_hover_text("Shadowmask also bakes Mixed lights' static shadows (next bake)");
}
