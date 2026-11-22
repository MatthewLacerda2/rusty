use super::tests::{batched, item};
use super::DrawItem;

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
    let draws = batched((opaque, transparent), true);
    assert_eq!(draws.outline.len(), 2);
    for (outline, tag) in draws.outline.iter().zip([1.0, 3.0]) {
        let batch = draws
            .batches()
            .find(|b| b.instances.contains(&outline.instance))
            .unwrap();
        assert_eq!(
            super::tests::tag_of(&draws.instances[outline.instance as usize]),
            tag
        );
        assert_eq!(outline.uniform_slot, batch.uniform_slot);
        assert_eq!(
            (&outline.mesh, outline.indices.clone()),
            (&batch.key.mesh, 0..36)
        );
        assert_eq!(outline.material, batch.key.material);
    }
    assert_eq!(draws.opaque.len(), 2);
}
