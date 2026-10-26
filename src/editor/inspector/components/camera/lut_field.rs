//! The Color Lookup's LUT path field (#1441). The path is written when the field
//! is left, not per keystroke, so the renderer never tries, and logs, every
//! half-typed path. Left means any way: Enter, a click away, or the card no
//! longer drawn (another entity selected, an asset opened), which egui reports as
//! no focus loss at all. Escape reverts.
//!
//! While the field is focused its text is a [`Draft`] in egui's memory, stamped
//! with the pass that drew it; [`commit_abandoned`], called by the inspector every
//! pass, writes a draft whose field was not drawn last pass.

use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::{PostProcessingComponent, Scene};

/// The text being typed, for which entity, and the last pass that drew it.
#[derive(Clone)]
struct Draft {
    entity: u32,
    text: String,
    pass: u64,
}

fn key() -> egui::Id {
    egui::Id::new("color_lookup_draft")
}

/// The field, on `id`'s volume `vc`.
pub(super) fn draw(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let saved = vc.color_lookup.lut.clone().unwrap_or_default();
    let draft = ui.data(|d| d.get_temp::<Draft>(key()));
    let mut text = match draft {
        Some(d) if d.entity == id => d.text,
        _ => saved.clone(),
    };
    let field = ui
        .horizontal(|ui| {
            ui.label("    LUT:");
            ui.text_edit_singleline(&mut text)
                .on_hover_text("A .cube, or a Unity strip .png (256×16, 1024×32), under assets/")
        })
        .inner;
    if field.has_focus() {
        let pass = ui.ctx().cumulative_pass_nr();
        ui.data_mut(|d| {
            d.insert_temp(
                key(),
                Draft {
                    entity: id,
                    text,
                    pass,
                },
            )
        });
        return;
    }
    if !field.lost_focus() {
        return;
    }
    ui.data_mut(|d| d.remove::<Draft>(key()));
    let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if !escaped && text != saved {
        *is_dirty |= commit(world, id, text);
    }
}

/// Write a draft whose field was not drawn last pass (its card was left), so a
/// path typed and then abandoned by selecting something else still lands.
/// Returns whether the scene changed.
pub fn commit_abandoned(ctx: &egui::Context, scene: &mut Scene) -> bool {
    let Some(draft) = ctx.data(|d| d.get_temp::<Draft>(key())) else {
        return false;
    };
    if draft.pass + 1 >= ctx.cumulative_pass_nr() {
        return false;
    }
    ctx.data_mut(|d| d.remove::<Draft>(key()));
    let saved = scene
        .world
        .post_processing(draft.entity)
        .and_then(|vc| vc.color_lookup.lut.clone());
    saved.unwrap_or_default() != draft.text && commit(&mut scene.world, draft.entity, draft.text)
}

/// Set `id`'s LUT path to `text`; false when it has no volume any more.
fn commit(world: &mut crate::ecs::World, id: u32, text: String) -> bool {
    let Some(mut c) = world.post_processing_mut(id) else {
        return false;
    };
    vc_ops::set_lut(&mut c, Some(text));
    true
}
