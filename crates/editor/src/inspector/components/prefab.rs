use egui_phosphor::regular as icon;

use crate::inspector::components::card::component_card;
use crate::theme;
use rusty_components::PART_KEY;
use rusty_edit::authoring;
use rusty_edit::prefab::instance;
use rusty_scene::Scene;

/// Dispatched once the entity guard drops: the verbs need `&mut Scene`. Whole-object
/// verbs take the instance root; per-field ones resolve it from `entity`.
pub enum PrefabAction {
    /// `Scene.RevertPrefabOverrides`.
    Revert(u32),
    /// `Scene.ReimportPrefab`.
    Reimport(u32),
    /// `Scene.ApplyPrefabToSource`.
    ApplyToSource(u32),
    /// `Scene.RevertPrefabField`.
    RevertField { entity: u32, path: String },
    /// `Scene.ApplyPrefabFieldToSource`.
    ApplyFieldToSource { entity: u32, path: String },
}

/// Draw the prefab-instance card for the entity `id`, if it is a linked instance.
/// `root` is resolved by the caller, since finding it needs the scene.
pub fn draw(
    ui: &mut egui::Ui,
    world: &rusty_ecs::World,
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

#[derive(Clone, Copy)]
struct Apply(Option<&'static str>);

impl Apply {
    const MODEL: &'static str = "A model is read-only: its instance's overrides can't be applied to the file (save it as a prefab first)";

    fn of(source: &str) -> Self {
        Apply(rusty_edit::prefab::source::is_model_source(source).then_some(Self::MODEL))
    }

    fn button(self, ui: &mut egui::Ui, button: egui::Button<'_>, hover: &str) -> bool {
        let response = ui.add_enabled(self.0.is_none(), button);
        match self.0 {
            Some(reason) => response.on_disabled_hover_text(reason).clicked(),
            None => response.on_hover_text(hover).clicked(),
        }
    }
}

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

fn labeled(ui: &mut egui::Ui, glyph: &str, label: &str, hover: &str) -> bool {
    ui.button(format!("{glyph}  {label}"))
        .on_hover_text(hover)
        .clicked()
}

fn draw_overrides_list(
    ui: &mut egui::Ui,
    world: &rusty_ecs::World,
    id: u32,
    apply: Apply,
    action: &mut Option<PrefabAction>,
) {
    let t = theme::from_ui(ui);
    let paths = rusty_edit::prefab::live::entity_overrides(world, id).keys();
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
        // A removed part is recorded, not diffed: there is no value to apply.
        let part = path.starts_with(PART_KEY);
        if !part && apply.button(ui, one, "Apply this field to the source .prefab") {
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

fn field_label(path: &str) -> String {
    if let Some(node) = path.strip_prefix(PART_KEY) {
        return format!("{node} (removed part)");
    }
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

pub fn instance_root(scene: &Scene, selected_id: u32) -> u32 {
    instance::instance_root(scene, selected_id).unwrap_or(selected_id)
}

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
