//! src/editor/inspector_prefab.rs — the linked-prefab-instance override card (#263).
//!
//! The editor half of prefab instances (#216 / #260): for an entity that carries a
//! [`crate::scene::PrefabLink`], it surfaces — Unity-style — which fields diverge from
//! the linked source and lets the author Apply / Revert them. Every button maps onto
//! the SAME verbs the `Scene.*` API and console drive (`record_instance_overrides` /
//! `revert_instance_overrides` / `reimport_instance`), so there is no second code path.
//!
//! The card is the deferred-action sort the rest of the inspector uses: it only reads
//! the live entity (the override map lives on its `prefab_link`) and emits a
//! [`PrefabAction`] the caller dispatches once the entity guard drops, where `&mut
//! Scene` is reachable for the verbs.

use egui_phosphor::regular as icon;

use crate::editor::inspector_card::component_card;
use crate::editor::theme;
use crate::scene::{authoring, Entity, Scene};

/// A prefab-override edit the card requested this frame, dispatched by the caller once
/// the entity guard has dropped (the verbs need `&mut Scene`). All whole-object verbs
/// take the instance ROOT id; a per-field revert also carries the entity + json-pointer
/// path so exactly that one leaf drops back to the source.
pub enum PrefabAction {
    /// Record the instance's current edits as overrides (`Scene.ApplyPrefabChanges`).
    Apply(u32),
    /// Drop every override and rebuild from the source (`Scene.RevertPrefabOverrides`).
    Revert(u32),
    /// Re-baseline from the source, re-applying overrides (`Scene.ReimportPrefab`).
    Reimport(u32),
    /// Revert one overridden field: drop `path` from `entity`'s override map, then
    /// reimport the instance rooted at `root` so that leaf tracks the source again.
    RevertField {
        root: u32,
        entity: u32,
        path: String,
    },
}

/// Draw the prefab-instance card for `entity`, if it is a linked instance. `root` is
/// the instance's root id (the entity carrying `local_id == 0`), pre-resolved by the
/// caller because finding it needs the scene. Returns the requested action, if any.
pub fn draw(ui: &mut egui::Ui, entity: &Entity, root: u32) -> Option<PrefabAction> {
    let link = entity.prefab_link.as_ref()?;
    let mut action = None;
    let title = format!("{}  Prefab", icon::LINK_SIMPLE);
    component_card(ui, "", &title, None, |ui| {
        let t = theme::from_ui(ui);
        ui.label(format!("Source: {}", link.source));
        ui.add_space(t.space_xs);
        action = draw_object_buttons(ui, root);
        draw_overrides_list(ui, entity, root, &mut action);
    });
    action
}

/// The whole-object Apply / Revert / Reimport row. Apply records the instance's edits;
/// Revert drops every override; Reimport re-pulls the source under the overrides.
fn draw_object_buttons(ui: &mut egui::Ui, root: u32) -> Option<PrefabAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        if ui
            .button(format!("{}  Apply All", icon::ARROW_SQUARE_UP))
            .on_hover_text("Record this instance's edits as overrides (Scene.ApplyPrefabChanges)")
            .clicked()
        {
            action = Some(PrefabAction::Apply(root));
        }
        if ui
            .button(format!("{}  Revert All", icon::ARROW_COUNTER_CLOCKWISE))
            .on_hover_text("Drop every override and rebuild from the source")
            .clicked()
        {
            action = Some(PrefabAction::Revert(root));
        }
        if ui
            .button(format!("{}  Reimport", icon::ARROWS_CLOCKWISE))
            .on_hover_text("Re-pull the source, keeping overrides")
            .clicked()
        {
            action = Some(PrefabAction::Reimport(root));
        }
    });
    action
}

/// The per-field override list: one row per overridden leaf on `entity`, each with the
/// Unity blue "modified" dot, a readable field label, and a per-field Revert button.
/// Shows a muted "no overrides" line when the instance entity matches the source.
fn draw_overrides_list(
    ui: &mut egui::Ui,
    entity: &Entity,
    root: u32,
    action: &mut Option<PrefabAction>,
) {
    let t = theme::from_ui(ui);
    let paths: Vec<String> = entity
        .prefab_link
        .as_ref()
        .map(|l| l.overrides.keys().cloned().collect())
        .unwrap_or_default();
    ui.separator();
    if paths.is_empty() {
        ui.colored_label(t.text_secondary, "No overrides on this object");
        return;
    }
    ui.label("Overridden fields:");
    for path in paths {
        draw_override_row(ui, t.accent_blue, entity.id, root, &path, action);
    }
}

/// One override row: the blue dot, the field label, and a per-field Revert button.
fn draw_override_row(
    ui: &mut egui::Ui,
    blue: egui::Color32,
    entity: u32,
    root: u32,
    path: &str,
    action: &mut Option<PrefabAction>,
) {
    ui.horizontal(|ui| {
        ui.colored_label(blue, icon::CIRCLE);
        ui.label(field_label(path));
        if ui
            .small_button(icon::ARROW_COUNTER_CLOCKWISE)
            .on_hover_text("Revert this field to the source")
            .clicked()
        {
            *action = Some(PrefabAction::RevertField {
                root,
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

/// The instance root for `selected_id`: walk up the parent chain to the topmost entity
/// that shares the selected entity's prefab source (`local_id == 0` anchors it).
/// Returns `selected_id` itself when it is not a linked instance, so callers can pass
/// the result unconditionally.
pub fn instance_root(scene: &Scene, selected_id: u32) -> u32 {
    let Some(source) = scene
        .get_entity(selected_id)
        .and_then(|e| e.prefab_link.as_ref().map(|l| l.source.clone()))
    else {
        return selected_id;
    };
    let mut current = selected_id;
    while let Some(parent) = scene.get_entity(current).and_then(|e| e.parent_id) {
        let parent_linked = scene
            .get_entity(parent)
            .map(|e| e.prefab_link.as_ref().is_some_and(|l| l.source == source))
            .unwrap_or(false);
        if !parent_linked {
            break;
        }
        current = parent;
    }
    current
}

/// Apply a [`PrefabAction`] against the live scene, routing through the shared
/// `scene::authoring` prefab verbs (the same ones the `Scene.*` API calls). A
/// per-field revert first drops the path from that entity's override map, then
/// reimports so the now-un-overridden leaf re-tracks the source. Returns whether the
/// scene changed (so the caller can mark the editor dirty).
pub fn dispatch(scene: &mut Scene, action: PrefabAction) -> bool {
    match action {
        PrefabAction::Apply(root) => authoring::record_instance_overrides(scene, root).is_ok(),
        PrefabAction::Revert(root) => authoring::revert_instance_overrides(scene, root).is_ok(),
        PrefabAction::Reimport(root) => authoring::reimport_instance(scene, root).is_ok(),
        PrefabAction::RevertField { root, entity, path } => {
            revert_field(scene, root, entity, &path)
        }
    }
}

/// Drop one override `path` from `entity`'s map, then reimport the instance so that
/// leaf returns to the source value while every other override is preserved.
fn revert_field(scene: &mut Scene, root: u32, entity: u32, path: &str) -> bool {
    let removed = scene
        .get_entity_mut(entity)
        .and_then(|mut e| e.prefab_link.as_mut().map(|l| l.overrides.remove(path)))
        .flatten()
        .is_some();
    if removed {
        let _ = authoring::reimport_instance(scene, root);
    }
    removed
}

#[cfg(test)]
#[path = "inspector_prefab_tests.rs"]
mod inspector_prefab_tests;
