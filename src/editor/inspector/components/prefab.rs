//! src/editor/inspector_prefab.rs — the linked-prefab-instance override card (#263).
//!
//! The editor half of prefab instances (#216 / #260 / #268): for an entity that carries
//! a [`crate::components::PrefabLink`], it surfaces — Unity-style — which fields diverge from
//! the linked source and lets the author Record / Revert them on the instance, or Apply
//! them back into the source `.prefab`. Every button maps onto the SAME verbs the
//! `Scene.*` API and console drive (`record_instance_overrides` /
//! `revert_instance_overrides` / `reimport_instance` / `apply_instance_to_source` /
//! `apply_instance_field_to_source` / `revert_instance_field`), so there is no second
//! code path.
//!
//! The card is the deferred-action sort the rest of the inspector uses: it only reads
//! the live entity (the override map lives on its `prefab_link`) and emits a
//! [`PrefabAction`] the caller dispatches once the entity guard drops, where `&mut
//! Scene` is reachable for the verbs.

use egui_phosphor::regular as icon;

use crate::editor::inspector::components::card::component_card;
use crate::editor::theme;
use crate::scene::prefab::instance;
use crate::scene::{authoring, Scene};

/// A prefab-override edit the card requested this frame, dispatched by the caller once
/// the entity guard has dropped (the verbs need `&mut Scene`). Whole-object verbs take
/// the instance ROOT id; the per-field verbs also carry the entity + json-pointer path
/// so exactly that one leaf is reverted / pushed to source.
pub enum PrefabAction {
    /// Record the instance's current edits as overrides (`Scene.RecordPrefabOverrides`).
    Record(u32),
    /// Drop every override and rebuild from the source (`Scene.RevertPrefabOverrides`).
    Revert(u32),
    /// Re-baseline from the source, re-applying overrides (`Scene.ReimportPrefab`).
    Reimport(u32),
    /// Write every override back into the source `.prefab`, then clear them on the
    /// instance (`Scene.ApplyPrefabToSource`).
    ApplyToSource(u32),
    /// Revert one overridden field so that leaf tracks the source again
    /// (`Scene.RevertPrefabField`). Like its apply sibling, the verb resolves the
    /// instance root from `entity`.
    RevertField { entity: u32, path: String },
    /// Write one overridden field back into the source `.prefab`, then clear it on the
    /// instance (`Scene.ApplyPrefabFieldToSource`). The verb resolves the instance root
    /// from `entity` and reimports it afterwards, so no separate `root` is needed.
    ApplyFieldToSource { entity: u32, path: String },
}

/// Draw the prefab-instance card for the entity `id`, if it is a linked instance.
/// `root` is the instance's root id (the entity carrying `local_id == 0`), pre-resolved
/// by the caller because finding it needs the scene. Returns the requested action, if
/// any.
pub fn draw(
    ui: &mut egui::Ui,
    world: &crate::ecs::World,
    id: u32,
    root: u32,
) -> Option<PrefabAction> {
    let source = world.prefab_link(id)?.source.clone();
    let mut action = None;
    let title = format!("{}  Prefab", icon::LINK_SIMPLE);
    component_card(ui, "", &title, None, |ui| {
        let t = theme::from_ui(ui);
        ui.label(format!("Source: {source}"));
        ui.add_space(t.space_xs);
        action = draw_object_buttons(ui, root);
        draw_overrides_list(ui, world, id, &mut action);
    });
    action
}

/// The whole-object button block. Top row is instance-side (Record edits as overrides,
/// Revert all, Reimport from source); the bottom row is the apply-to-source direction
/// (write every override back into the `.prefab`, then clear them on the instance).
fn draw_object_buttons(ui: &mut egui::Ui, root: u32) -> Option<PrefabAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        if labeled(
            ui,
            icon::FLOPPY_DISK,
            "Record",
            "Record this instance's edits as overrides (Scene.RecordPrefabOverrides)",
        ) {
            action = Some(PrefabAction::Record(root));
        }
        if labeled(
            ui,
            icon::ARROW_COUNTER_CLOCKWISE,
            "Revert All",
            "Drop every override and rebuild from the source",
        ) {
            action = Some(PrefabAction::Revert(root));
        }
        if labeled(
            ui,
            icon::ARROWS_CLOCKWISE,
            "Reimport",
            "Re-pull the source, keeping overrides",
        ) {
            action = Some(PrefabAction::Reimport(root));
        }
    });
    if labeled(ui, icon::ARROW_SQUARE_UP, "Apply All to Source", "Write every override back into the source .prefab, then clear them (Scene.ApplyPrefabToSource)") {
        action = Some(PrefabAction::ApplyToSource(root));
    }
    action
}

/// A labelled button with an icon and a hover tooltip; returns whether it was clicked.
/// Factored out so the object-button rows stay short and uniform.
fn labeled(ui: &mut egui::Ui, glyph: &str, label: &str, hover: &str) -> bool {
    ui.button(format!("{glyph}  {label}"))
        .on_hover_text(hover)
        .clicked()
}

/// The per-field override list: one row per overridden leaf on entity `id`, each with
/// the Unity blue "modified" dot, a readable field label, and a per-field Revert button.
/// Shows a muted "no overrides" line when the instance entity matches the source.
fn draw_overrides_list(
    ui: &mut egui::Ui,
    world: &crate::ecs::World,
    id: u32,
    action: &mut Option<PrefabAction>,
) {
    let t = theme::from_ui(ui);
    let paths: Vec<String> = world
        .prefab_link(id)
        .map(|l| l.overrides.keys().cloned().collect())
        .unwrap_or_default();
    ui.separator();
    if paths.is_empty() {
        ui.colored_label(t.text_secondary, "No overrides on this object");
        return;
    }
    ui.label("Overridden fields:");
    for path in paths {
        draw_override_row(ui, t.accent, id, &path, action);
    }
}

/// One override row: the blue dot, the field label, a per-field "apply to source"
/// button, and a per-field Revert button.
fn draw_override_row(
    ui: &mut egui::Ui,
    blue: egui::Color32,
    entity: u32,
    path: &str,
    action: &mut Option<PrefabAction>,
) {
    ui.horizontal(|ui| {
        ui.colored_label(blue, icon::CIRCLE);
        ui.label(field_label(path));
        if ui
            .small_button(icon::ARROW_SQUARE_UP)
            .on_hover_text("Apply this field to the source .prefab")
            .clicked()
        {
            *action = Some(PrefabAction::ApplyFieldToSource {
                entity,
                path: path.to_string(),
            });
        }
        if ui
            .small_button(icon::ARROW_COUNTER_CLOCKWISE)
            .on_hover_text("Revert this field to the source")
            .clicked()
        {
            *action = Some(PrefabAction::RevertField {
                entity,
                path: path.to_string(),
            });
        }
    });
}

/// Turn a json-pointer override path (e.g. `/transform/position/0`) into a friendly
/// inspector label (e.g. `transform / position / 0`). Purely cosmetic; the dispatched
/// action keeps the raw pointer.
fn field_label(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return path.to_string();
    }
    trimmed.replace('/', " / ")
}

/// The instance root for `selected_id` (`scene::prefab::instance::instance_root`, the
/// rule every prefab verb shares). Returns `selected_id` itself when it is not a linked
/// instance, so callers can pass the result unconditionally.
pub fn instance_root(scene: &Scene, selected_id: u32) -> u32 {
    instance::instance_root(scene, selected_id).unwrap_or(selected_id)
}

/// Apply a [`PrefabAction`] against the live scene, routing through the shared
/// `scene::authoring` prefab verbs (the same ones the `Scene.*` API calls). Returns
/// whether the scene changed (so the caller can mark the editor dirty).
pub fn dispatch(scene: &mut Scene, action: PrefabAction) -> bool {
    match action {
        PrefabAction::Record(root) => authoring::record_instance_overrides(scene, root).is_ok(),
        PrefabAction::Revert(root) => authoring::revert_instance_overrides(scene, root).is_ok(),
        PrefabAction::Reimport(root) => authoring::reimport_instance(scene, root).is_ok(),
        PrefabAction::ApplyToSource(root) => {
            authoring::apply_instance_to_source(scene, root).is_ok()
        }
        PrefabAction::RevertField { entity, path } => {
            authoring::revert_instance_field(scene, entity, &path).unwrap_or(false)
        }
        PrefabAction::ApplyFieldToSource { entity, path } => {
            authoring::apply_instance_field_to_source(scene, entity, &path).is_ok()
        }
    }
}

#[cfg(test)]
#[path = "prefab_tests.rs"]
mod inspector_prefab_tests;
