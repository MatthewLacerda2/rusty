//! src/editor/inspector/components/prefab.rs — the linked-prefab-instance override card (#263).
//!
//! The editor half of prefab instances (#216 / #260 / #268): for an entity that carries
//! a [`crate::components::PrefabLink`], it surfaces — Unity-style — which fields diverge from
//! the linked source and lets the author Revert them on the instance, or Apply them
//! back into the source `.prefab` (greyed out, with the reason on hover, when the
//! source is a model, which is read-only, #1637). An edit is an override the moment it is made
//! (#1527), so there is nothing to record. Every button maps onto the SAME verbs the
//! `Scene.*` API and console drive (`revert_instance_overrides` / `reimport_instance` / `apply_instance_to_source` /
//! `apply_instance_field_to_source` / `revert_instance_field`), so there is no second
//! code path.
//!
//! The card is the deferred-action sort the rest of the inspector uses: it only reads
//! the live entity (its overrides are its diff against its link's baseline) and emits a
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
        let apply = Apply::of(&source);
        action = draw_object_buttons(ui, root, apply);
        draw_overrides_list(ui, world, id, apply, &mut action);
    });
    action
}

/// Whether the card's apply-to-source buttons can act: a model source is read-only
/// (#1637), so Unity greys out Apply on a model prefab instance, and so does this.
#[derive(Clone, Copy)]
struct Apply(Option<&'static str>);

impl Apply {
    /// Why the buttons are off.
    const MODEL: &'static str = "A model is read-only: its instance's overrides can't be applied to the file (save it as a prefab first)";

    /// The apply state for an instance of `source`.
    fn of(source: &str) -> Self {
        Apply(crate::scene::prefab::source::is_model_source(source).then_some(Self::MODEL))
    }

    /// Add `button`, enabled with `hover` as its tooltip, or greyed out with the
    /// reason it is off; returns whether it was clicked.
    fn button(self, ui: &mut egui::Ui, button: egui::Button<'_>, hover: &str) -> bool {
        let response = ui.add_enabled(self.0.is_none(), button);
        match self.0 {
            Some(reason) => response.on_disabled_hover_text(reason).clicked(),
            None => response.on_hover_text(hover).clicked(),
        }
    }
}

/// The whole-object button block. Top row is instance-side (Revert all, Reimport from
/// source); the bottom row is the apply-to-source direction
/// (write every override back into the `.prefab`, then clear them on the instance).
fn draw_object_buttons(ui: &mut egui::Ui, root: u32, apply: Apply) -> Option<PrefabAction> {
    let mut action = None;
    ui.horizontal(|ui| {
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
    let all = egui::Button::new(format!("{}  Apply All to Source", icon::ARROW_SQUARE_UP));
    if apply.button(ui, all, "Write every override back into the source .prefab, then clear them (Scene.ApplyPrefabToSource)") {
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
    apply: Apply,
    action: &mut Option<PrefabAction>,
) {
    let t = theme::from_ui(ui);
    let paths = crate::scene::prefab::live::entity_overrides(world, id).keys();
    ui.separator();
    if paths.is_empty() {
        ui.colored_label(t.text_secondary, "No overrides on this object");
        return;
    }
    ui.label("Overridden fields:");
    for path in paths {
        draw_override_row(ui, t.accent, (id, &path), apply, action);
    }
}

/// One override row: the blue dot, the field label, a per-field "apply to source"
/// button, and a per-field Revert button.
fn draw_override_row(
    ui: &mut egui::Ui,
    blue: egui::Color32,
    (entity, path): (u32, &str),
    apply: Apply,
    action: &mut Option<PrefabAction>,
) {
    ui.horizontal(|ui| {
        ui.colored_label(blue, icon::CIRCLE);
        ui.label(field_label(path));
        let one = egui::Button::new(icon::ARROW_SQUARE_UP).small();
        if apply.button(ui, one, "Apply this field to the source .prefab") {
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

/// Turn an override key into a friendly inspector label: a json pointer
/// (`/transform/position/0`) reads `transform / position / 0`, an added component
/// (`+/light`) `light (added)` and a removed one (`-/light`) `light (removed)`.
/// Purely cosmetic; the dispatched action keeps the raw key.
fn field_label(path: &str) -> String {
    if let Some(component) = path.strip_prefix("+/") {
        return format!("{component} (added)");
    }
    if let Some(component) = path.strip_prefix("-/") {
        return format!("{component} (removed)");
    }
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
