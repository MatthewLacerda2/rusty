//! src/editor/inspector_context.rs — the inspector's read-only frame context.
//!
//! Everything the entity inspector needs that requires reading the WHOLE scene, gathered
//! up front: the layer labels/mask set, the parenting matrix + name + valid-parent list,
//! and the linked-prefab instance root. It is collected before `inspector.rs` takes the
//! mutable entity guard (which borrows the scene for the rest of the frame), so these
//! whole-scene reads can't fight that borrow.

use crate::editor::inspector_prefab;
use crate::scene::{Scene, LAYER_COUNT};

/// Read-only context the inspector needs that borrows the whole `Scene`, gathered
/// up front because the entity guard borrows it mutably for the rest of the frame.
pub(crate) struct InspectorContext {
    pub layer_labels: Vec<String>,
    pub named_layers: Vec<(u8, String)>,
    pub parent_mat: Option<glam::Mat4>,
    pub selected_parent_name: String,
    pub valid_parents: Vec<(u32, String)>,
    /// The root id of the linked-prefab instance the selection belongs to (the entity
    /// itself when it is not a linked instance). Resolved here because walking up to it
    /// needs read access to the whole scene, gathered before the entity guard is taken.
    pub prefab_root: u32,
}

/// Gather the read-only layer labels, parenting matrix/name, valid-parent set, and the
/// prefab instance root for the selected entity (see [`InspectorContext`]).
pub(crate) fn gather_context(scene: &Scene, selected_id: u32) -> InspectorContext {
    // The layer dropdown labels (one per registry slot).
    let layer_labels: Vec<String> = (0..LAYER_COUNT)
        .map(|i| scene.layers.label(i as u8))
        .collect();

    // Layers offered in the camera culling-mask checklist: layer 0 plus any named
    // user slots (unnamed slots are noise, mirroring Unity's mask dropdown).
    let named_layers: Vec<(u8, String)> = (0..LAYER_COUNT as u8)
        .filter(|&i| i == 0 || !scene.layers.name(i).is_empty())
        .map(|i| (i, scene.layers.label(i)))
        .collect();

    let current_parent_id = scene.get_entity(selected_id).and_then(|e| e.parent_id);
    let parent_mat = current_parent_id.map(|p| scene.compute_world_matrix(p));
    let selected_parent_name = if let Some(p_id) = current_parent_id {
        scene
            .get_entity(p_id)
            .map(|e| e.name.clone())
            .unwrap_or("None".to_string())
    } else {
        "None".to_string()
    };

    InspectorContext {
        layer_labels,
        named_layers,
        parent_mat,
        selected_parent_name,
        valid_parents: collect_valid_parents(scene, selected_id),
        prefab_root: inspector_prefab::instance_root(scene, selected_id),
    }
}

/// The entities that may be the selected entity's parent: everything except
/// itself and its own descendants (which would create a cycle).
fn collect_valid_parents(scene: &Scene, selected_id: u32) -> Vec<(u32, String)> {
    scene
        .iter()
        .filter(|e| e.id != selected_id)
        .map(|e| (e.id, e.name.clone()))
        .filter(|(id, _)| {
            let mut curr = *id;
            while let Some(ancestor) = scene.get_entity(curr).and_then(|x| x.parent_id) {
                if ancestor == selected_id {
                    return false;
                }
                curr = ancestor;
            }
            true
        })
        .collect()
}
