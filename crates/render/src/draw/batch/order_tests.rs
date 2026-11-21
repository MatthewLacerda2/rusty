use super::sort::depth_bucket;
use super::tests::{at, batched, item, meshes, name, tags};

#[test]
fn layered_walls_draw_nearest_first_whatever_the_given_order() {
    let opaque = vec![
        at("Far", 0.0, 40.0),
        at("Mid", 1.0, 20.0),
        at("Near", 2.0, 5.0),
    ];
    let draws = batched((opaque, Vec::new()), true);
    assert_eq!(meshes(&draws), ["Near", "Mid", "Far"]);
}

#[test]
fn a_run_draws_at_its_nearest_instance_and_stays_one_draw() {
    let opaque = vec![
        at("Crate", 0.0, 50.0),
        at("Wall", 1.0, 10.0),
        at("Crate", 2.0, 60.0),
        at("Crate", 3.0, 2.0),
    ];
    let draws = batched((opaque, Vec::new()), true);
    assert_eq!(meshes(&draws), ["Crate", "Wall"]);
    assert_eq!(draws.opaque[0].instances.len(), 3);
    let tags: Vec<_> = draws.instances.iter().map(super::tests::tag_of).collect();
    assert_eq!(tags, [3.0, 0.0, 2.0, 1.0]);
    // Uniform slots follow the draw order, so `pack()` still lines up.
    let slots: Vec<_> = draws.opaque.iter().map(|b| b.uniform_slot).collect();
    assert_eq!(slots, [0, 1]);
}

#[test]
fn near_equal_depths_keep_the_key_order() {
    // Within one bucket the material (key) order holds, so state changes stay few.
    assert_eq!(depth_bucket(10.0), depth_bucket(10.5));
    let opaque = vec![at("B", 0.0, 10.0), at("A", 1.0, 10.5)];
    let draws = batched((opaque, Vec::new()), true);
    assert_eq!(meshes(&draws), ["A", "B"]);
}

#[test]
fn the_material_never_reorders_meshes_inside_a_bucket() {
    let with = |mesh, material, tag| {
        let mut it = at(mesh, tag, 10.0);
        it.key.material = material;
        it
    };
    let opaque = vec![with("A", 1, 0.0), with("B", 0, 1.0), with("A", 0, 2.0)];
    let draws = batched((opaque, Vec::new()), true);
    let order: Vec<_> = draws
        .opaque
        .iter()
        .map(|b| (name(b.key.mesh), b.key.material))
        .collect();
    assert_eq!(order, [("A", 0), ("A", 1), ("B", 0)]);
}

#[test]
fn buckets_grow_with_distance_and_clamp_at_the_camera() {
    assert!(depth_bucket(1.0) < depth_bucket(2.0));
    assert!(depth_bucket(100.0) < depth_bucket(200.0));
    // Beside or behind the camera, and a NaN, all count as nearest.
    let nearest = depth_bucket(0.0);
    assert_eq!(depth_bucket(-30.0), nearest);
    assert_eq!(depth_bucket(f32::NAN), nearest);
    assert!(nearest < depth_bucket(1.0));
}

#[test]
fn without_instancing_the_given_order_is_kept() {
    let opaque = vec![at("Far", 0.0, 40.0), at("Near", 1.0, 5.0)];
    let draws = batched((opaque, Vec::new()), false);
    assert_eq!(meshes(&draws), ["Far", "Near"]);
}

#[test]
fn instancing_off_is_one_draw_per_entity_in_scene_order() {
    let opaque = vec![
        item("Box", 0, 1.0, 0.0),
        item("Barrel", 0, 1.0, 1.0),
        item("Box", 0, 1.0, 2.0),
    ];
    let draws = batched((opaque, Vec::new()), false);
    assert_eq!(draws.opaque.len(), 3);
    assert_eq!(tags(&draws), [0.0, 1.0, 2.0]);
    assert!(draws.opaque.iter().all(|b| b.instances.len() == 1));
}
