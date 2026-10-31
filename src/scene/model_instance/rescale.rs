//! A Scale Factor change reaching a whole model's instances (#1083, #1398).
//!
//! An instance's node offsets were copied from the import into its child
//! Transforms once, so a re-import at a new factor rescales the meshes and rigs
//! (`rehydrate`) but not where the parts sit. Unity updates every instance of a
//! re-imported model; here each instance is found from its re-imported meshes and
//! its node entities' local offsets are scaled by the factor's ratio. An instance
//! is matched by the spawn's own rule (`super::holder`, `lod::group_name`), node
//! by node by name: a child renamed or moved out of its place is left where it is.

use std::collections::BTreeSet;

use crate::asset::ModelNode;
use crate::scene::Scene;

/// Scale by `ratio` the node offsets of every whole-model instance (of a model
/// whose node tree is `nodes`) that holds one of `meshes`, the entities its
/// re-import rebuilt. The instance root keeps its place. Returns how many
/// instances moved.
pub fn rescale_node_offsets(
    scene: &mut Scene,
    nodes: &[ModelNode],
    meshes: &[u32],
    ratio: f32,
) -> usize {
    if !ratio.is_finite() || ratio <= 0.0 || ratio == 1.0 || nodes.is_empty() {
        return 0;
    }
    let mut covered: BTreeSet<u32> = BTreeSet::new();
    let mut moved = 0;
    for &mesh in meshes {
        if covered.contains(&mesh) {
            continue;
        }
        let mut up = scene.world.parent_id(mesh);
        while let Some(root) = up {
            let placed = locate(scene, root, nodes);
            if placed.contains(&mesh) {
                for &id in &placed {
                    if let Some(mut t) = scene.world.transform_mut(id) {
                        t.position *= ratio;
                    }
                }
                covered.extend(placed);
                moved += 1;
                break;
            }
            up = scene.world.parent_id(root);
        }
    }
    moved
}

/// The entities under `root` that the spawn placed by `nodes`' offsets, matched
/// the way it built them: each node's entity, and each LODGroup's.
fn locate(scene: &Scene, root: u32, nodes: &[ModelNode]) -> BTreeSet<u32> {
    let mut placed = BTreeSet::new();
    let mut entity: Vec<Option<u32>> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let parent = match node.parent {
            Some(p) => entity.get(p).copied().flatten(),
            None => Some(root),
        };
        let found = parent.and_then(|parent| {
            let (holder, _) = super::holder(scene, node, parent);
            let within = match super::lod::group_name(node) {
                Some(name) => {
                    // Every member of one group finds the same group.
                    let group = child(scene, holder, &name, &BTreeSet::new())
                        .filter(|&g| scene.world.lod_group(g).is_some())?;
                    placed.insert(group);
                    group
                }
                None => holder,
            };
            child(scene, within, &node.name, &placed)
        });
        placed.extend(found);
        entity.push(found);
    }
    placed
}

/// A child of `parent` named `name` not already in `taken`.
fn child(scene: &Scene, parent: u32, name: &str, taken: &BTreeSet<u32>) -> Option<u32> {
    let children = scene.world.children(parent).into_iter();
    children
        .filter(|c| !taken.contains(c))
        .find(|&c| scene.world.name(c).is_some_and(|n| *n == name))
}
