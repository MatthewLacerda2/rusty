//! A model re-import refits a linked prefab instance's mesh collider (#1398) as the
//! model's change, not the author's (#1527): it is no override, and the instance's
//! own edits stay overrides.

use glam::Vec3;

use super::{instantiate_asset, set_model_scale_factor};
use crate::components::ColliderShape;
use crate::scene::asset_instance::lightmap_test_support::quad_model;
use crate::scene::authoring::defaults::default_collider;
use crate::scene::{list_instance_overrides, load_and_instantiate_linked, save_prefab, Scene};

#[test]
fn a_refit_mesh_collider_is_no_override() {
    let model = quad_model("rusty_1527_refit");
    let mut author = Scene::new();
    let quad = instantiate_asset(&mut author, &format!("{model}::Quad"), None, Vec3::ZERO);
    let quad = quad.unwrap();
    let mut collider = default_collider();
    collider.shape = ColliderShape::Mesh {
        convex: true,
        local_min: Vec3::ZERO,
        local_max: Vec3::ZERO,
    };
    author.world.set_collider(quad, Some(collider));
    let path = crate::test_temp::dir().join("rusty_1527_refit.prefab");
    let path = path.to_string_lossy().into_owned();
    save_prefab(&author, quad, &path).unwrap();

    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    scene.world.transform_mut(root).unwrap().scale = Vec3::splat(2.0);
    assert_eq!(set_model_scale_factor(&mut scene, &model, 0.01), Ok(1));

    let shape = scene.world.collider(root).unwrap().shape.clone();
    let ColliderShape::Mesh { local_max, .. } = shape else {
        panic!("a mesh collider, got {shape:?}");
    };
    assert!(
        local_max.abs_diff_eq(Vec3::new(0.01, 0.01, 0.0), 1e-7),
        "refit"
    );
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert_eq!(
        listed,
        [
            "0:/transform/scale/0",
            "0:/transform/scale/1",
            "0:/transform/scale/2"
        ]
    );
}
