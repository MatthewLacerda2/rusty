//! The inspector's deferred edits: the ones a card asks for during the draw but
//! that need `&mut Scene`, applied once the card pass is done. The navmesh is not
//! one of them: the editor syncs it once per edit-mode frame (#992).

use super::components::material::{self, MaterialAction};
use super::components::mesh::SkinTool;
use super::components::prefab;
use crate::editor::EditorUi;
use crate::scene::skeleton::{HitboxOptions, RagdollOptions};
use crate::scene::Scene;

/// Edits the per-frame card draw defers until the mutable entity guard has dropped,
/// because each needs `&mut Scene` that the guard's borrow would otherwise block. Collected during the draw, then applied by [`Self::apply`].
#[derive(Default)]
pub(super) struct PendingEdits {
    pub(super) parent_change: Option<Option<u32>>,
    pub(super) prefab_action: Option<prefab::PrefabAction>,
    /// The Mesh card's *Generate Hitboxes* / *Build Ragdoll* buttons (#464, #466).
    pub(super) skin_tool: Option<SkinTool>,
    /// The Material card's *Delete* (#1576).
    pub(super) material_action: Option<MaterialAction>,
}

impl PendingEdits {
    /// Apply the deferred edits once the entity guard has dropped: re-parent, run a
    /// prefab verb, delete or restore a material, run a skin tool, and mark the scene dirty for each that did.
    pub(super) fn apply(self, editor: &mut EditorUi, scene: &mut Scene, id: u32) {
        if let Some(action) = self.prefab_action {
            if prefab::dispatch(scene, action) {
                editor.is_dirty = true;
            }
        }
        if let Some(action) = self.material_action {
            if material::apply_action(editor, scene, action) {
                editor.is_dirty = true;
            }
        }
        if let Some(new_parent) = self.parent_change {
            // A reparent moves the subtree in world space; the write queues its
            // collider bounds itself (#992, `ecs::world_space::bounds`).
            if scene.set_parent(id, new_parent).is_ok() {
                editor.is_dirty = true;
            }
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
        match (self.skin_tool, built) {
            (Some(_), Ok(())) => editor.is_dirty = true,
            (Some(tool), Err(e)) => log::warn!("[Editor] {tool:?}: {e}"),
            (None, _) => {}
        }
    }
}
