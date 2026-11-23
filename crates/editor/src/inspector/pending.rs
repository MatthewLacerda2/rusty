//! The inspector's deferred edits: the ones a card asks for during the draw but
//! that need `&mut Scene`, applied once the card pass is done. The navmesh is not
//! one of them: the editor syncs it once per edit-mode frame.

use super::components::material::{self, MaterialAction};
use super::components::mesh::SkinTool;
use super::components::prefab;
use crate::EditorUi;
use rusty_scene::skeleton::{HitboxOptions, RagdollOptions};
use rusty_scene::Scene;

#[derive(Default)]
pub(super) struct PendingEdits {
    pub(super) parent_change: Option<Option<u32>>,
    pub(super) prefab_action: Option<prefab::PrefabAction>,
    /// The Mesh card's *Generate Hitboxes* / *Build Ragdoll* buttons.
    pub(super) skin_tool: Option<SkinTool>,
    /// The Material card's *Delete*.
    pub(super) material_action: Option<MaterialAction>,
}

impl PendingEdits {
    /// Apply the deferred edits once the entity guard has dropped: re-parent, run a
    /// prefab verb, delete or restore a material, run a skin tool.
    pub(super) fn apply(self, editor: &mut EditorUi, scene: &mut Scene, id: u32) {
        if let Some(action) = self.prefab_action {
            prefab::dispatch(scene, action);
        }
        if let Some(action) = self.material_action {
            material::apply_action(editor, scene, action);
        }
        if let Some(new_parent) = self.parent_change {
            // A reparent moves the subtree in world space; the write queues its
            // collider bounds itself. A refused one changes nothing.
            let _ = scene.reparent(id, new_parent);
        }
        let built = match self.skin_tool {
            Some(SkinTool::Hitboxes) => scene
                .generate_hitboxes(id, &HitboxOptions::default())
                .map(drop),
            Some(SkinTool::Ragdoll) => scene
                .build_ragdoll(id, &RagdollOptions::default())
                .map(drop),
            None => Ok(()),
        };
        if let (Some(tool), Err(e)) = (self.skin_tool, built) {
            log::warn!("[Editor] {tool:?}: {e}");
        }
    }
}
