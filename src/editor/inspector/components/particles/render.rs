//! src/editor/inspector/components/particles/render.rs — the card's Rendering
//! section: blend, texture, and the #440 render settings (mode, stretch, mesh,
//! flipbook, soft, lit). Every write goes through a shared particle op.

use super::widgets::{clamped, combo, drag_u32};
use super::{apply, Cx};
use crate::components::{Flipbook, ParticleRenderMode};
use crate::scene::authoring::particles as particle_ops;
use crate::scene::{ParticleBlend, ParticleEmitterComponent};

/// Blend, texture, render mode and its mode-specific settings, then the look.
pub(super) fn draw(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut changed = draw_blend(ui, cx, p);
    let mut mode = p.render.mode;
    let modes = ParticleRenderMode::ALL.map(|m| (m, m.name()));
    if combo(ui, "Render Mode", &mut mode, &modes) {
        apply(cx, |c| particle_ops::set_render_mode(c, mode));
        changed = true;
    }
    match p.render.mode {
        ParticleRenderMode::Stretched => changed |= draw_stretch(ui, cx, p),
        ParticleRenderMode::Mesh => changed |= draw_mesh(ui, cx, p),
        _ => {}
    }
    changed | draw_look(ui, cx, p)
}

/// Blend mode and the optional sprite texture path.
fn draw_blend(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    let mut blend = p.blend;
    let blends = [
        (ParticleBlend::Alpha, "Alpha"),
        (ParticleBlend::Additive, "Additive"),
    ];
    if combo(ui, "Blend", &mut blend, &blends) {
        apply(cx, |c| particle_ops::set_blend(c, blend));
        changed = true;
    }
    if let Some(tex) = text_row(ui, "Texture:", p.texture.as_deref()) {
        apply(cx, |c| particle_ops::set_texture(c, tex));
        changed = true;
    }
    changed
}

/// Stretched: length per unit of size, and per unit of speed.
fn draw_stretch(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let (mut length, mut speed) = (p.render.length_scale, p.render.speed_scale);
    let changed = clamped(ui, "Length Scale:", &mut length, 0.0..=100.0)
        | clamped(ui, "Speed Scale:", &mut speed, 0.0..=10.0);
    if changed {
        apply(cx, |c| particle_ops::set_stretch(c, length, speed));
    }
    changed
}

/// Mesh: the mesh (primitive name or model path) and the scene material.
fn draw_mesh(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut changed = false;
    if let Some(mesh) = text_row(ui, "Mesh:", p.render.mesh.as_deref()) {
        apply(cx, |c| particle_ops::set_render_mesh(c, mesh));
        changed = true;
    }
    if let Some(material) = text_row(ui, "Material:", p.render.material.as_deref()) {
        apply(cx, |c| particle_ops::set_render_material(c, material));
        changed = true;
    }
    changed
}

/// Flipbook grid, soft-fade distance and the lit toggle.
fn draw_look(ui: &mut egui::Ui, cx: &mut Cx<'_>, p: &ParticleEmitterComponent) -> bool {
    let mut book: Flipbook = p.render.flipbook;
    let mut changed = drag_u32(ui, "Flipbook Columns:", &mut book.columns, 1..=64)
        | drag_u32(ui, "Flipbook Rows:", &mut book.rows, 1..=64);
    if book.frames() > 1 {
        changed |= clamped(ui, "Cycles:", &mut book.cycles, 0.0..=100.0);
        changed |= ui
            .checkbox(&mut book.random_start, "Random Start Frame")
            .changed();
    }
    if changed {
        apply(cx, |c| particle_ops::set_flipbook(c, book));
    }
    let mut soft = p.render.soft_distance;
    if clamped(ui, "Soft Distance:", &mut soft, 0.0..=10.0) {
        apply(cx, |c| particle_ops::set_soft_distance(c, soft));
        changed = true;
    }
    let mut lit = p.render.lit;
    if ui.checkbox(&mut lit, "Lit").changed() {
        apply(cx, |c| particle_ops::set_lit(c, lit));
        changed = true;
    }
    changed
}

/// A labelled single-line text row; `Some(new text)` when edited.
fn text_row(ui: &mut egui::Ui, label: &str, value: Option<&str>) -> Option<String> {
    let mut text = value.unwrap_or_default().to_string();
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(&mut text).changed()
    })
    .inner
    .then_some(text)
}
