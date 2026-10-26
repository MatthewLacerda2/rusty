//! The card's file path fields: the Color Lookup's LUT (#1441) and Bloom's lens
//! dirt (#1442), one [`PathField`] each in [`FIELDS`]. A path is written when its
//! field is left, not per keystroke, so the renderer never tries, and logs, every
//! half-typed path. Left means any way: Enter, a click away, or the card no longer
//! drawn (another entity selected, an asset opened), which egui reports as no focus
//! loss at all. Escape reverts.
//!
//! While a field is focused its text is a [`Draft`] in egui's memory, stamped with
//! the pass that drew it; [`commit_abandoned`], called by the inspector every pass,
//! writes a draft whose field was not drawn last pass.

use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::{PostProcessingComponent, Scene};

/// One path field: what it is called, what it reads and the op that writes it.
pub(super) struct PathField {
    /// Names its draft in egui's memory.
    pub id: &'static str,
    pub label: &'static str,
    pub hover: &'static str,
    pub get: fn(&PostProcessingComponent) -> Option<String>,
    pub set: fn(&mut PostProcessingComponent, Option<String>),
}

/// The Color Lookup's LUT file.
pub(super) const LUT: PathField = PathField {
    id: "color_lookup_draft",
    label: "    LUT:",
    hover: "A .cube, or a Unity strip .png (256×16, 1024×32), under assets/",
    get: |vc| vc.color_lookup.lut.clone(),
    set: vc_ops::grading::color_lookup::set_lut,
};

/// Bloom's lens dirt texture.
pub(super) const DIRT: PathField = PathField {
    id: "bloom_dirt_draft",
    label: "    Dirt Texture:",
    hover: "An image under assets/, or engine/textures/lens_dirt.png, the engine's own",
    get: |vc| vc.bloom.dirt_texture.clone(),
    set: vc_ops::bloom::set_dirt_texture,
};

/// Every path field, for [`commit_abandoned`].
const FIELDS: &[&PathField] = &[&DIRT, &LUT];

/// The text being typed, for which entity, and the last pass that drew it.
#[derive(Clone)]
struct Draft {
    entity: u32,
    text: String,
    pass: u64,
}

fn key(field: &PathField) -> egui::Id {
    egui::Id::new(field.id)
}

/// `field`, on `id`'s volume `vc`.
pub(super) fn draw(
    ui: &mut egui::Ui,
    (world, id): (&mut crate::ecs::World, u32),
    field: &PathField,
    vc: &PostProcessingComponent,
    is_dirty: &mut bool,
) {
    let saved = (field.get)(vc).unwrap_or_default();
    let draft = ui.data(|d| d.get_temp::<Draft>(key(field)));
    let mut text = match draft {
        Some(d) if d.entity == id => d.text,
        _ => saved.clone(),
    };
    let edit = ui
        .horizontal(|ui| {
            ui.label(field.label);
            ui.text_edit_singleline(&mut text)
                .on_hover_text(field.hover)
        })
        .inner;
    if edit.has_focus() {
        let pass = ui.ctx().cumulative_pass_nr();
        let draft = Draft {
            entity: id,
            text,
            pass,
        };
        ui.data_mut(|d| d.insert_temp(key(field), draft));
        return;
    }
    if !edit.lost_focus() {
        return;
    }
    ui.data_mut(|d| d.remove::<Draft>(key(field)));
    let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if !escaped && text != saved {
        *is_dirty |= commit(world, id, field, text);
    }
}

/// Write each draft whose field was not drawn last pass (its card was left), so a
/// path typed and then abandoned by selecting something else still lands.
/// Returns whether the scene changed.
pub fn commit_abandoned(ctx: &egui::Context, scene: &mut Scene) -> bool {
    let mut changed = false;
    for field in FIELDS {
        let Some(draft) = ctx.data(|d| d.get_temp::<Draft>(key(field))) else {
            continue;
        };
        if draft.pass + 1 >= ctx.cumulative_pass_nr() {
            continue;
        }
        ctx.data_mut(|d| d.remove::<Draft>(key(field)));
        let saved = scene
            .world
            .post_processing(draft.entity)
            .and_then(|vc| (field.get)(&*vc));
        if saved.unwrap_or_default() != draft.text {
            changed |= commit(&mut scene.world, draft.entity, field, draft.text);
        }
    }
    changed
}

/// Set `id`'s `field` to `text`; false when it has no volume any more.
fn commit(world: &mut crate::ecs::World, id: u32, field: &PathField, text: String) -> bool {
    let Some(mut c) = world.post_processing_mut(id) else {
        return false;
    };
    (field.set)(&mut c, Some(text));
    true
}
