//! The outline's draw is the forward draw's (#1175): an item marked `outline` names
//! the instance and uniform slot it was packed into, wherever batching sorted it.
//! No GPU.

use super::tests::item;
use super::{DrawItem, FrameDraws};

/// `item`, marked for the outline.
fn ringed(mut item: DrawItem) -> DrawItem {
    item.outline = true;
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
        assert_eq!(
            (&outline.mesh, outline.indices.clone()),
            (&batch.key.mesh, 0..36)
        );
        assert_eq!(outline.material, batch.key.material);
    }
    // The ringed box still shares the boxes' one instanced draw.
    assert_eq!(draws.opaque.len(), 2);
}
