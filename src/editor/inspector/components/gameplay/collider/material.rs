//! src/editor/inspector/components/gameplay/collider/material.rs — the Collider
//! card's Physics Material section (#1878): the shared asset the collider names (a
//! project path; empty for inline values), then the values. With an asset, an edit
//! writes the asset, retuning every collider that uses it, as Unity's inspector
//! edits a `PhysicMaterial` asset; without one, it writes the inline values.
//!
//! Two things are held in egui's memory between passes, and [`commit_abandoned`],
//! called by the inspector every pass, lands what the card left behind:
//!
//! - **The path being typed** is a [`Draft`], as the post-processing card's path
//!   fields keep theirs (`camera::post_processing::path_field`): written when the
//!   field is left (Enter, a click away, or the card no longer drawn), not per
//!   keystroke; Escape reverts. A path no asset loads from is refused and warned of.
//! - **An asset edit under a drag** is a [`Pending`]: shown in the card, written to
//!   the file once the drag ends. A file write is an atomic write, a write-log note
//!   and a generation bump, so it is not made sixty times a second.

use crate::components::{ColliderComponent, CombineMode, PhysicsMaterial};
use crate::ecs::World;
use crate::editor::widgets::DragRange;
use crate::scene::authoring::physics::collider as collider_ops;
use crate::scene::physics_material;
use crate::scene::Scene;

/// The asset path being typed, for which entity, and the last pass that drew it.
#[derive(Clone)]
struct Draft {
    entity: u32,
    text: String,
    pass: u64,
}

/// Asset values edited under a drag, not yet written to `asset`.
#[derive(Clone)]
struct Pending {
    asset: String,
    material: PhysicsMaterial,
}

fn draft_key() -> egui::Id {
    egui::Id::new("collider_material_asset_draft")
}

fn pending_key() -> egui::Id {
    egui::Id::new("collider_material_pending")
}

/// The section, for `id`'s `collider`.
pub(super) fn draw(
    ui: &mut egui::Ui,
    world: &mut World,
    id: u32,
    collider: &ColliderComponent,
    is_dirty: &mut bool,
) {
    ui.separator();
    ui.label("Physics Material");
    if asset_field(ui, world, id, collider) {
        *is_dirty = true;
        return;
    }
    let Some(asset) = &collider.material_asset else {
        if let Some(material) = draw_values(ui, collider.material) {
            if let Some(mut c) = world.collider_mut(id) {
                collider_ops::set_material(&mut c, material);
            }
            *is_dirty = true;
        }
        return;
    };
    physics_material::poll();
    let pending = ui
        .data(|d| d.get_temp::<Pending>(pending_key()))
        .filter(|p| &p.asset == asset);
    let shown = match pending {
        Some(p) => p.material,
        None => match physics_material::lookup(asset) {
            Some(m) => m,
            None => {
                ui.label("Missing: the default material is used");
                return;
            }
        },
    };
    if let Some(material) = draw_values(ui, shown) {
        let pending = Pending {
            asset: asset.clone(),
            material,
        };
        ui.data_mut(|d| d.insert_temp(pending_key(), pending));
    }
    flush_pending(ui.ctx());
}

/// The Asset path field; true when it changed the collider's asset.
fn asset_field(ui: &mut egui::Ui, world: &mut World, id: u32, c: &ColliderComponent) -> bool {
    let saved = c.material_asset.clone().unwrap_or_default();
    let mut text = match ui.data(|d| d.get_temp::<Draft>(draft_key())) {
        Some(d) if d.entity == id => d.text,
        _ => saved.clone(),
    };
    let edit = ui
        .horizontal(|ui| {
            ui.label("Asset:");
            ui.text_edit_singleline(&mut text)
                .on_hover_text("A .physmat under assets/; empty for this collider's own values")
        })
        .inner;
    if edit.has_focus() {
        let pass = ui.ctx().cumulative_pass_nr();
        let draft = Draft {
            entity: id,
            text,
            pass,
        };
        ui.data_mut(|d| d.insert_temp(draft_key(), draft));
        return false;
    }
    if !edit.lost_focus() {
        return false;
    }
    ui.data_mut(|d| d.remove::<Draft>(draft_key()));
    let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
    !escaped && text != saved && commit(world, id, text)
}

/// Write the abandoned path draft (its field was not drawn last pass) and an
/// asset edit whose drag has ended. Returns whether the scene changed.
pub fn commit_abandoned(ctx: &egui::Context, scene: &mut Scene) -> bool {
    flush_pending(ctx);
    let Some(draft) = ctx.data(|d| d.get_temp::<Draft>(draft_key())) else {
        return false;
    };
    if draft.pass + 1 >= ctx.cumulative_pass_nr() {
        return false;
    }
    ctx.data_mut(|d| d.remove::<Draft>(draft_key()));
    let saved = scene
        .world
        .collider(draft.entity)
        .and_then(|c| c.material_asset.clone());
    saved.unwrap_or_default() != draft.text && commit(&mut scene.world, draft.entity, draft.text)
}

/// Name `text` as `id`'s asset; false when it has no collider or the path is
/// refused (warned of).
fn commit(world: &mut World, id: u32, text: String) -> bool {
    let Some(mut c) = world.collider_mut(id) else {
        return false;
    };
    match collider_ops::set_material_asset(&mut c, Some(text)) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("[Physics] Collider asset: {e}");
            false
        }
    }
}

/// Write the pending asset edit once nothing is being dragged.
fn flush_pending(ctx: &egui::Context) {
    if ctx.dragged_id().is_some() {
        return;
    }
    let Some(p) = ctx.data(|d| d.get_temp::<Pending>(pending_key())) else {
        return;
    };
    ctx.data_mut(|d| d.remove::<Pending>(pending_key()));
    if physics_material::lookup(&p.asset) == Some(p.material) {
        return;
    }
    if let Err(e) = physics_material::write(&p.asset, p.material) {
        log::warn!("[Physics] {e}");
    }
}

/// Friction, bounciness and a combine mode for each (Unity's `PhysicMaterial`).
/// Edits a copy; returns the new material when any widget changed it.
fn draw_values(ui: &mut egui::Ui, mut m: PhysicsMaterial) -> Option<PhysicsMaterial> {
    let mut changed = ui
        .horizontal(|ui| {
            ui.label("Friction:");
            let mut c = ui
                .add(
                    egui::DragValue::new(&mut m.friction)
                        .speed(0.01)
                        .ranged(0.0..=10.0),
                )
                .changed();
            ui.label("Bounciness:");
            c |= ui
                .add(
                    egui::DragValue::new(&mut m.bounciness)
                        .speed(0.01)
                        .ranged(0.0..=1.0),
                )
                .changed();
            c
        })
        .inner;
    changed |= combine_combo(ui, "Friction Combine", &mut m.friction_combine);
    changed |= combine_combo(ui, "Bounce Combine", &mut m.bounce_combine);
    changed.then_some(m)
}

/// A combine-mode combo box; returns whether the mode changed.
fn combine_combo(ui: &mut egui::Ui, label: &str, mode: &mut CombineMode) -> bool {
    let before = *mode;
    egui::ComboBox::from_label(label)
        .selected_text(mode.name())
        .show_ui(ui, |ui| {
            for m in CombineMode::ALL {
                ui.selectable_value(mode, m, m.name());
            }
        });
    *mode != before
}
