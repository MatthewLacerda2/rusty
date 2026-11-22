use glam::Vec3;

use super::probe::read;
use super::trace_gpu_tests::level;
use crate::Renderer;
use rusty_components::{CameraComponent, RenderMode};
use rusty_scene::Scene;

/// Refreshes the tree; checks it against a fresh one and its GPU copy against it, and
/// returns (top rebuilds, mesh builds).
fn step(r: &mut Renderer, scene: &Scene) -> (u32, u32) {
    r.frame_counters = Default::default();
    r.extract_scene(scene);
    r.rays_kept_equal_fresh(scene).unwrap();
    let tree = r.ray_tree(scene).expect("extracted");
    let packed = tree.packed();
    let gpu = tree.gpu.as_ref().expect("uploaded");
    let [_, nodes, tris, _, instances, faces] = gpu.buffers();
    let (nodes, tris) = (nodes.clone(), tris.clone());
    let (instances, faces) = (instances.clone(), faces.clone());
    let prefix = |bytes: Vec<u8>, want: &[u8]| bytes[..want.len()] == *want;
    assert!(prefix(read(r, &nodes), bytemuck::cast_slice(&packed.nodes)));
    assert!(prefix(read(r, &tris), bytemuck::cast_slice(&packed.tris)));
    assert!(prefix(
        read(r, &instances),
        bytemuck::cast_slice(&packed.instances)
    ));
    assert!(prefix(read(r, &faces), bytemuck::cast_slice(&packed.faces)));
    let c = r.frame_counters;
    (c.ray_tree_rebuilds, c.ray_mesh_builds)
}

#[test]
fn gpu_the_kept_tree_equals_a_fresh_one_after_each_edit() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut scene = level();
    let ids = scene.world.ids_with_mesh();
    assert_eq!(
        step(&mut r, &scene),
        (1, 2),
        "a first build: the plane and the box"
    );
    assert_eq!(
        step(&mut r, &scene),
        (0, 0),
        "nothing changed, nothing rebuilt"
    );
    scene.world.transform_mut(ids[3]).unwrap().position += Vec3::Y;
    assert_eq!(
        step(&mut r, &scene),
        (1, 0),
        "a static move rebuilds only the top"
    );
    scene.world.set_static(ids[4], false);
    assert_eq!(step(&mut r, &scene).0, 1);
    scene.world.despawn(ids[5]);
    assert_eq!(step(&mut r, &scene).0, 1);
    if let Some(mut mesh) = scene.world.mesh_mut(ids[1]) {
        mesh.vertices_mut()
            .iter_mut()
            .for_each(|v| v.position[1] *= 2.0);
    }
    assert_eq!(
        step(&mut r, &scene),
        (1, 1),
        "an edited mesh builds its own tree"
    );
    scene.materials.get_mut("both").unwrap().render_mode = RenderMode::Transparent;
    step(&mut r, &scene);
    let none = r.ray_tree(&scene).unwrap().packed();
    assert!(
        none.instances.len() < ids.len() - 9,
        "transparent cubes are not traced"
    );
    let before = none.instances.len();
    scene.world.set_layer(ids[0], 3);
    let cam = rusty_scene::create::create_entity(&mut scene, "Eye", None);
    let mut camera = CameraComponent::default();
    camera.culling_mask = !(1 << 3);
    scene.world.set_camera(cam, Some(camera));
    step(&mut r, &scene);
    let after = r.ray_tree(&scene).unwrap().packed().instances.len();
    assert_eq!(after, before - 1, "a layer no camera draws is left out");
}
