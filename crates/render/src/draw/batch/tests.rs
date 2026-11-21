use super::{BatchKey, DrawItem, FrameDraws, OpaqueOrder, RecordTraits};
use crate::scene::MaterialRecords;
use crate::scene::MeshHandle;
use crate::InstanceData;

/// The mesh names these tests draw, sorted: a name's handle is its index, so
/// handles order as the names do.
const MESHES: [&str; 10] = [
    "A", "B", "Barrel", "Box", "Crate", "Far", "Glass", "Mid", "Near", "Wall",
];

pub(super) fn handle(name: &str) -> MeshHandle {
    let at = MESHES.iter().position(|&m| m == name);
    MeshHandle(at.unwrap_or_else(|| panic!("add {name} to MESHES")) as u32)
}

pub(super) fn name(handle: MeshHandle) -> &'static str {
    MESHES[handle.0 as usize]
}

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
            depth_only: false,
        },
        instance,
        depth: 0.0,
        outline: false,
    }
}

pub(super) fn at(mesh: &str, tag: f32, depth: f32) -> DrawItem {
    DrawItem {
        depth,
        ..item(mesh, 0, 1.0, tag)
    }
}

/// `items` (opaque, transparent) batched with no material classes, the opaque
/// runs ranked by `order`, instanced when `instancing`.
pub(super) fn ordered(
    items: (Vec<DrawItem>, Vec<DrawItem>),
    how: (bool, OpaqueOrder),
) -> FrameDraws {
    FrameDraws::build(items, &MaterialRecords::default(), how)
}

pub(super) fn batched(items: (Vec<DrawItem>, Vec<DrawItem>), instancing: bool) -> FrameDraws {
    ordered(items, (instancing, OpaqueOrder::Unranked))
}

/// The mesh of each opaque draw, in draw order.
pub(super) fn meshes(draws: &FrameDraws) -> Vec<&str> {
    draws.opaque.iter().map(|b| name(b.key.mesh)).collect()
}

pub(super) fn tag_of(instance: &InstanceData) -> f32 {
    f32::from_bits(instance.object)
}

pub(super) fn tags(draws: &FrameDraws) -> Vec<f32> {
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
    let draws = batched((opaque, Vec::new()), true);
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
    let draws = batched((opaque, Vec::new()), true);
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
    let draws = batched((vec![item("Box", 0, 1.0, 9.0)], transparent), true);
    let runs: Vec<_> = draws
        .transparent
        .iter()
        .map(|b| (name(b.key.mesh), b.instances.clone()))
        .collect();
    assert_eq!(runs, [("A", 1..3), ("B", 3..4), ("A", 4..5)]);
    assert_eq!(tags(&draws), [9.0, 0.0, 1.0, 2.0, 3.0]);
    assert_eq!(draws.transparent[0].uniform_slot, 1);
}
