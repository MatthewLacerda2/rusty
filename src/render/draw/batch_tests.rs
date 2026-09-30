//! The instancing grouping rules (#470), pinned without a GPU.

use super::{BatchKey, DrawItem, FrameDraws};
use crate::render::{InstanceData, MeshId};

/// An item of mesh `mesh`, palette `bone_base`, tint `tint`, tagged by `tag` in its
/// instance's translation so a test can read back the order instances were packed in.
fn item(mesh: &str, bone_base: u32, tint: f32, tag: f32) -> DrawItem {
    let mut uniform = [0u32; 36];
    uniform[16] = tint.to_bits();
    uniform[34] = bone_base;
    let mut instance = InstanceData::IDENTITY;
    instance.model_matrix[12] = tag;
    DrawItem {
        key: BatchKey {
            mesh: MeshId(mesh.to_string()),
            material: 0,
            uniform,
        },
        num_indices: 36,
        instance,
    }
}

fn tags(draws: &FrameDraws) -> Vec<f32> {
    draws.instances.iter().map(|i| i.model_matrix[12]).collect()
}

#[test]
fn copies_of_one_mesh_and_material_become_one_draw() {
    let opaque = vec![
        item("Box", 0, 1.0, 0.0),
        item("Barrel", 0, 1.0, 1.0),
        item("Box", 0, 1.0, 2.0),
        item("Box", 0, 1.0, 3.0),
    ];
    let draws = FrameDraws::build(opaque, Vec::new(), true);
    let runs: Vec<_> = draws
        .opaque
        .iter()
        .map(|b| (b.key.mesh.0.as_str(), b.instances.clone()))
        .collect();
    assert_eq!(runs, [("Barrel", 0..1), ("Box", 1..4)]);
    // Stable within a run: the boxes keep their scene order.
    assert_eq!(tags(&draws), [1.0, 0.0, 2.0, 3.0]);
}

#[test]
fn a_different_uniform_or_skinned_palette_splits_the_batch() {
    let opaque = vec![
        item("Box", 0, 1.0, 0.0),
        item("Box", 0, 0.5, 1.0),
        item("Box", 1, 1.0, 2.0),
        item("Box", 2, 1.0, 3.0),
    ];
    let draws = FrameDraws::build(opaque, Vec::new(), true);
    assert_eq!(
        draws.opaque.len(),
        4,
        "tint, and each skinned palette, is its own draw"
    );
    let slots: Vec<u32> = draws.opaque.iter().map(|b| b.uniform_slot).collect();
    assert_eq!(slots, [0, 1, 2, 3]);
    assert_eq!(draws.uniforms().len(), 4);
}

#[test]
fn transparent_order_is_kept_and_only_neighbours_merge() {
    // Back-to-front: glass A, A, then B, then A again — the last A must not jump the B.
    let transparent = vec![
        item("A", 0, 1.0, 0.0),
        item("A", 0, 1.0, 1.0),
        item("B", 0, 1.0, 2.0),
        item("A", 0, 1.0, 3.0),
    ];
    let draws = FrameDraws::build(vec![item("Box", 0, 1.0, 9.0)], transparent, true);
    let runs: Vec<_> = draws
        .transparent
        .iter()
        .map(|b| (b.key.mesh.0.as_str(), b.instances.clone()))
        .collect();
    assert_eq!(runs, [("A", 1..3), ("B", 3..4), ("A", 4..5)]);
    assert_eq!(tags(&draws), [9.0, 0.0, 1.0, 2.0, 3.0]);
    // Uniform slots continue after the opaque batches.
    assert_eq!(draws.transparent[0].uniform_slot, 1);
}

#[test]
fn instancing_off_is_one_draw_per_entity_in_scene_order() {
    let opaque = vec![
        item("Box", 0, 1.0, 0.0),
        item("Barrel", 0, 1.0, 1.0),
        item("Box", 0, 1.0, 2.0),
    ];
    let draws = FrameDraws::build(opaque, Vec::new(), false);
    assert_eq!(draws.opaque.len(), 3);
    assert_eq!(tags(&draws), [0.0, 1.0, 2.0]);
    assert!(draws.opaque.iter().all(|b| b.instances.len() == 1));
}
