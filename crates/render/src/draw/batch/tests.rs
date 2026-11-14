//! The instancing grouping rules (#470), pinned without a GPU.

use super::{BatchKey, DrawItem, FrameDraws, RecordTraits};
use crate::scene::MaterialRecords;
use crate::scene::MeshHandle;
use crate::InstanceData;

/// The mesh names these tests draw, sorted: a name's handle is its index, so
/// handles order as the names do.
const MESHES: [&str; 10] = [
    "A", "B", "Barrel", "Box", "Crate", "Far", "Glass", "Mid", "Near", "Wall",
];

/// The handle of test mesh `name`.
pub(super) fn handle(name: &str) -> MeshHandle {
    let at = MESHES.iter().position(|&m| m == name);
    MeshHandle(at.unwrap_or_else(|| panic!("add {name} to MESHES")) as u32)
}

/// The name of test mesh `handle`.
pub(super) fn name(handle: MeshHandle) -> &'static str {
    MESHES[handle.0 as usize]
}

/// An item of mesh `mesh`, palette `bone_base`, tint `tint` (its material record's
/// slot is ten times it), tagged by `tag` (its bits as the instance's object slot)
/// so a test can read back the order instances were packed in.
pub(super) fn item(mesh: &str, bone_base: u32, tint: f32, tag: f32) -> DrawItem {
    let record = (tint * 10.0) as u32;
    let traits = RecordTraits::of(&crate::MaterialData::flat([tint; 4]));
    let instance = InstanceData::of(tag.to_bits());
    DrawItem {
        key: BatchKey {
            pipeline: 0,
            facing: Default::default(),
            mesh: handle(mesh),
            indices: (0..36).into(),
            material: 0,
            record,
            traits,
            bone_base,
        },
        instance,
        depth: 0.0,
        outline: false,
    }
}

/// The tag [`item`] gave `instance`.
pub(super) fn tag_of(instance: &InstanceData) -> f32 {
    f32::from_bits(instance.object)
}

fn tags(draws: &FrameDraws) -> Vec<f32> {
    draws.instances.iter().map(tag_of).collect()
}

#[test]
fn copies_of_one_mesh_and_material_become_one_draw() {
    let opaque = vec![
        item("Box", 0, 1.0, 0.0),
        item("Barrel", 0, 1.0, 1.0),
        item("Box", 0, 1.0, 2.0),
        item("Box", 0, 1.0, 3.0),
    ];
    let draws = FrameDraws::build((opaque, Vec::new()), &MaterialRecords::default(), true);
    let runs: Vec<_> = draws
        .opaque
        .iter()
        .map(|b| (name(b.key.mesh), b.instances.clone()))
        .collect();
    assert_eq!(runs, [("Barrel", 0..1), ("Box", 1..4)]);
    // Stable within a run: at one depth, the boxes keep their scene order.
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
    let draws = FrameDraws::build((opaque, Vec::new()), &MaterialRecords::default(), true);
    assert_eq!(
        draws.opaque.len(),
        4,
        "tint, and each skinned palette, is its own draw"
    );
    let slots: Vec<u32> = draws.opaque.iter().map(|b| b.uniform_slot).collect();
    assert_eq!(slots, [0, 1, 2, 3]);
    let uniforms = draws.pack();
    assert_eq!(uniforms.len(), 4);
    let named: Vec<_> = uniforms.iter().map(|u| (u.material, u.bone_base)).collect();
    assert_eq!(
        named,
        [(5, 0), (10, 0), (10, 1), (10, 2)],
        "each uniform names its key's record slot"
    );
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
    let draws = FrameDraws::build(
        (vec![item("Box", 0, 1.0, 9.0)], transparent),
        &MaterialRecords::default(),
        true,
    );
    let runs: Vec<_> = draws
        .transparent
        .iter()
        .map(|b| (name(b.key.mesh), b.instances.clone()))
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
    let draws = FrameDraws::build((opaque, Vec::new()), &MaterialRecords::default(), false);
    assert_eq!(draws.opaque.len(), 3);
    assert_eq!(tags(&draws), [0.0, 1.0, 2.0]);
    assert!(draws.opaque.iter().all(|b| b.instances.len() == 1));
}
