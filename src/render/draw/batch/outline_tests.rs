//! The outline's draw is the forward draw's (#1175): an item marked `outline` names
//! the instance and uniform slot it was packed into, wherever batching sorted it,
//! and its winding twin follows the matrices the vertex stage multiplies. No GPU.

use glam::{Mat4, Vec3};

use super::tests::item;
use super::{DrawItem, FrameDraws};

/// `item`, marked for the outline.
fn ringed(mut item: DrawItem) -> DrawItem {
    item.outline = true;
    item
}

/// `item`, ringed, its draw transform the identity (as a solid's) and its instance
/// placed by `world`.
fn ringed_at(item: DrawItem, world: Mat4) -> DrawItem {
    let mut item = ringed(item);
    let identity = Mat4::IDENTITY.to_cols_array().map(f32::to_bits);
    item.key.uniform[..16].copy_from_slice(&identity);
    item.instance.model_matrix = world.to_cols_array();
    item
}

#[test]
fn an_outlined_item_names_its_own_instance_and_slot() {
    let opaque = vec![
        item("Box", 0, 1.0, 0.0),
        ringed(item("Box", 0, 1.0, 1.0)),
        item("Barrel", 0, 1.0, 2.0),
    ];
    let transparent = vec![ringed(item("Glass", 0, 1.0, 3.0))];
    let draws = FrameDraws::build(opaque, transparent, true);
    assert_eq!(draws.outline.len(), 2);
    for (outline, tag) in draws.outline.iter().zip([1.0, 3.0]) {
        let batch = draws
            .batches()
            .find(|b| b.instances.contains(&outline.instance))
            .unwrap();
        assert_eq!(
            draws.instances[outline.instance as usize].model_matrix[12],
            tag
        );
        assert_eq!(outline.uniform_slot, batch.uniform_slot);
        assert_eq!((&outline.mesh, outline.num_indices), (&batch.key.mesh, 36));
    }
    // The ringed box still shares the boxes' one instanced draw.
    assert_eq!(draws.opaque.len(), 2);
}

#[test]
fn a_mirrored_instance_outlines_through_the_clockwise_twin() {
    let flip = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let mirrored = ringed_at(item("Box", 0, 1.0, 0.0), flip);
    let plain = ringed_at(item("Box", 0, 1.0, 1.0), Mat4::IDENTITY);
    let draws = FrameDraws::build(vec![mirrored, plain], Vec::new(), true);
    let twins: Vec<bool> = draws.outline.iter().map(|o| o.mirrored).collect();
    assert_eq!(twins, [true, false]);
}
