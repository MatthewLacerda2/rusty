//! The inspector's deferred edits: the ones a card asks for during the draw but
//! that need `&mut Scene` (or the navmesh), applied once the card pass is done.

use super::components::prefab;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::skeleton::HitboxOptions;
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
    /// The Mesh card's *Generate Hitboxes* button (#464).
    pub(super) generate_hitboxes: bool,
}

impl PendingEdits {
    /// Apply the deferred edits once the entity guard has dropped: re-parent, run a
    /// prefab verb, generate hitboxes, mark dirty, and re-bake the navmesh as requested.
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
        if self.generate_hitboxes {
            match scene.generate_hitboxes(id, &HitboxOptions::default()) {
                Ok(_) => editor.is_dirty = true,
                Err(e) => log::warn!("[Editor] Generate Hitboxes: {e}"),
            }
        }
        if self.nav_bake {
            // Incremental when only colliders/obstacles changed (#456).
            nav.sync(scene);
        }
    }
}
