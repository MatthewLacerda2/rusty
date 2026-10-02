//! The inspector's deferred edits: the ones a card asks for during the draw but
//! that need `&mut Scene` (or the navmesh), applied once the card pass is done.

use super::components::prefab;
use super::components::render::SkinTool;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::skeleton::{HitboxOptions, RagdollOptions};
use crate::scene::Scene;

/// Edits the per-frame card draw defers until the mutable entity guard has dropped,
/// because each needs `&mut Scene` (or `&mut NavigationGraph`) that the guard's borrow
/// would otherwise block. Collected during the draw, then applied by [`Self::apply`].
#[derive(Default)]
pub(super) struct PendingEdits {
    pub(super) parent_change: Option<Option<u32>>,
    pub(super) nav_bake: bool,
    pub(super) layer_changed: bool,
    pub(super) prefab_action: Option<prefab::PrefabAction>,
    /// The Mesh card's *Generate Hitboxes* / *Build Ragdoll* buttons (#464, #466).
    pub(super) skin_tool: Option<SkinTool>,
}

impl PendingEdits {
    /// Apply the deferred edits once the entity guard has dropped: re-parent, run a
    /// prefab verb, run a skin tool, mark dirty, and re-bake the navmesh as requested.
    pub(super) fn apply(
        self,
        editor: &mut EditorUi,
        scene: &mut Scene,
        nav: &mut NavigationGraph,
        id: u32,
    ) {
        if self.layer_changed {
            editor.is_dirty = true;
        }
        if let Some(action) = self.prefab_action {
            if prefab::dispatch(scene, action) {
                editor.is_dirty = true;
            }
        }
        if let Some(new_parent) = self.parent_change {
            let _ = scene.set_parent(id, new_parent);
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
        if self.nav_bake {
            // Incremental when only colliders/obstacles changed (#456).
            nav.sync(scene);
        }
    }
}
