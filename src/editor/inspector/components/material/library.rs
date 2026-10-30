//! The Material card's *Delete* (#1576): take the material the card edits out of
//! the scene's library, as deleting a material asset in Unity does. A click asks
//! for a confirm that lists the material's users; confirming deletes it through
//! the shared `authoring::material::delete_asset` op `Material.DeleteAsset` calls
//! (with `force`, so each user falls back to its default material), and keeps one
//! *Undo* step that puts the asset and every user it moved back.
//!
//! Listing users and deleting need the whole `Scene` (live decals are users), so
//! the card only emits a [`MaterialAction`], applied after the card pass. The
//! confirm and the *Undo* belong to the scene they were made in: a scene change
//! drops both ([`MaterialDelete::keep_scene`]).

use crate::editor::EditorUi;
use crate::scene::authoring::material::{self as mat_ops, DeletedMaterial};
use crate::scene::{Scene, SceneId};

/// The editor's delete state across frames.
#[derive(Default)]
pub struct MaterialDelete {
    /// The material waiting on the confirm.
    confirm: Option<Confirm>,
    /// The last delete, for *Undo*.
    undo: Option<DeletedMaterial>,
}

/// A delete waiting on its confirm.
struct Confirm {
    scene: SceneId,
    key: String,
    /// Its users, as the confirm lists them.
    users: Vec<String>,
}

impl MaterialDelete {
    /// Drop the confirm and the *Undo* when they were made in a scene other than
    /// `scene`, the one the inspector now shows.
    pub fn keep_scene(&mut self, scene: SceneId) {
        if self.confirm.as_ref().is_some_and(|c| c.scene != scene) {
            self.confirm = None;
        }
        if self.undo.as_ref().is_some_and(|u| u.scene != scene) {
            self.undo = None;
        }
    }
}

/// What a click on the card asked for.
pub enum MaterialAction {
    /// Show the confirm for this material.
    Ask(String),
    /// Delete this material.
    Delete(String),
    Cancel,
    Undo,
}

/// Draw the card's *Delete* row for library material `key`: the confirm while one
/// is up for `key`, else the delete button, and the *Undo* of the last delete
/// while it holds.
pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &MaterialDelete,
    key: &str,
    action: &mut Option<MaterialAction>,
) {
    match &state.confirm {
        Some(c) if c.key == key => draw_confirm(ui, key, &c.users, action),
        _ => {
            ui.horizontal(|ui| {
                ui.label(format!("Asset: {key}"));
                if ui.button("Delete").clicked() {
                    *action = Some(MaterialAction::Ask(key.to_string()));
                }
            });
        }
    }
    if let Some(undo) = &state.undo {
        if ui.button(format!("Undo delete of {}", undo.key)).clicked() {
            *action = Some(MaterialAction::Undo);
        }
    }
}

/// The confirm: the users the delete moves to their default material, and the two
/// answers.
fn draw_confirm(
    ui: &mut egui::Ui,
    key: &str,
    users: &[String],
    action: &mut Option<MaterialAction>,
) {
    ui.label(format!("Delete material {key}?"));
    if users.is_empty() {
        ui.label("Nothing uses it.");
    } else {
        ui.label("These move to their default material:");
        for user in users {
            ui.label(format!("• {user}"));
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Delete").clicked() {
            *action = Some(MaterialAction::Delete(key.to_string()));
        }
        if ui.button("Cancel").clicked() {
            *action = Some(MaterialAction::Cancel);
        }
    });
}

/// Apply `action` to `scene`; `true` when the library changed.
pub(in crate::editor::inspector) fn apply(
    editor: &mut EditorUi,
    scene: &mut Scene,
    action: MaterialAction,
) -> bool {
    let state = &mut editor.material_delete;
    match action {
        MaterialAction::Ask(key) => {
            let users = mat_ops::users(scene, &key);
            let users = users.iter().map(|u| mat_ops::describe(scene, u)).collect();
            let scene = scene.id();
            state.confirm = Some(Confirm { scene, key, users });
            false
        }
        MaterialAction::Cancel => {
            state.confirm = None;
            false
        }
        MaterialAction::Delete(key) => {
            state.confirm = None;
            let deleted = mat_ops::delete_asset(scene, &key, true);
            deleted
                .map(|d| state.undo = Some(d))
                .map_err(|e| log::warn!("[Editor] delete material: {e}"))
                .is_ok()
        }
        MaterialAction::Undo => {
            let Some(deleted) = state.undo.take() else {
                return false;
            };
            mat_ops::restore_asset(scene, deleted)
                .map_err(|e| log::warn!("[Editor] undo delete material: {e}"))
                .is_ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::MaterialComponent;

    #[test]
    fn ask_then_delete_then_undo_round_trips_the_library() {
        let mut editor = EditorUi::new(Default::default());
        let mut scene = Scene::new();
        scene.materials.insert("tile".into(), Default::default());
        let id = scene.add_entity("Floor".into());
        let used = Some(MaterialComponent::new("tile"));
        scene.world.set_material(id, used);

        let ask = MaterialAction::Ask("tile".into());
        assert!(!apply(&mut editor, &mut scene, ask));
        let confirm = editor.material_delete.confirm.as_ref().unwrap();
        assert_eq!(confirm.users, [format!("entity {id} \"Floor\" slot 0")]);

        let delete = MaterialAction::Delete("tile".into());
        assert!(apply(&mut editor, &mut scene, delete));
        assert!(editor.material_delete.confirm.is_none());
        assert!(!scene.materials.contains_key("tile"));

        assert!(apply(&mut editor, &mut scene, MaterialAction::Undo));
        assert!(scene.materials.contains_key("tile"));
        assert_eq!(scene.world.material(id).unwrap().first(), "tile");
        assert!(editor.material_delete.undo.is_none(), "one undo step");
    }

    #[test]
    fn a_scene_change_drops_the_confirm_and_the_undo() {
        let mut editor = EditorUi::new(Default::default());
        let mut scene = Scene::new();
        for key in ["tile", "spare"] {
            scene.materials.insert(key.into(), Default::default());
        }
        let delete = MaterialAction::Delete("spare".into());
        assert!(apply(&mut editor, &mut scene, delete));
        let ask = MaterialAction::Ask("tile".into());
        apply(&mut editor, &mut scene, ask);

        let state = &mut editor.material_delete;
        state.keep_scene(scene.id());
        assert!(
            state.confirm.is_some() && state.undo.is_some(),
            "same scene"
        );
        state.keep_scene(Scene::new().id());
        assert!(state.confirm.is_none() && state.undo.is_none());
    }
}
