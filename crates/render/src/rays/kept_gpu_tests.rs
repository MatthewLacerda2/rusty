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
    let buffers = gpu.buffers().map(Clone::clone);
    let want: [&[u8]; 6] = [
        bytemuck::bytes_of(&packed.header),
        bytemuck::cast_slice(&packed.nodes),
        bytemuck::cast_slice(&packed.tris),
        bytemuck::cast_slice(&packed.uvs),
        bytemuck::cast_slice(&packed.instances),
        bytemuck::cast_slice(&packed.faces),
    ];
    let names = ["header", "nodes", "triangles", "UVs", "instances", "faces"];
    for ((buffer, want), name) in buffers.iter().zip(want).zip(names) {
        let bytes = read(r, buffer);
        assert!(bytes[..want.len()] == *want, "the GPU's {name} differ");
    }
    let c = r.frame_counters;
    (c.ray_tree_rebuilds, c.ray_mesh_builds)
}

/// The tree's triangle buffer: the same one while only the top is uploaded again.
fn tris(r: &mut Renderer, scene: &Scene) -> wgpu::Buffer {
    let tree = r.ray_tree(scene).expect("extracted");
    tree.gpu.as_ref().expect("uploaded").buffers()[2].clone()
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
}

#[test]
fn gpu_hiding_and_showing_a_prop_uploads_only_the_top() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut scene = level();
    let ids = scene.world.ids_with_mesh();
    step(&mut r, &scene);
    let before = tris(&mut r, &scene);
    scene.world.set_active(ids[0], false);
    assert_eq!(
        step(&mut r, &scene),
        (1, 0),
        "hiding the plane keeps its mesh's tree"
    );
    scene.world.set_active(ids[0], true);
    assert_eq!(
        step(&mut r, &scene),
        (1, 0),
        "showing it again builds nothing"
    );
    assert!(
        tris(&mut r, &scene) == before,
        "and uploads only the top again"
    );
}

#[test]
fn gpu_what_no_camera_shows_opaque_is_left_out() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut scene = level();
    let ids = scene.world.ids_with_mesh();
    step(&mut r, &scene);
    scene.materials.get_mut("both").unwrap().render_mode = RenderMode::Transparent;
    step(&mut r, &scene);
    let none = r.ray_tree(&scene).unwrap().packed();
    assert_eq!(
        none.instances.len(),
        ids.len() - 9,
        "transparent cubes are not traced"
    );
    let before = none.instances.len();
    scene.world.set_layer(ids[0], 3);
    let cam = rusty_scene::create::create_entity(&mut scene, "Eye", None);
    let camera = CameraComponent {
        culling_mask: !(1 << 3),
        ..Default::default()
    };
    scene.world.set_camera(cam, Some(camera));
    step(&mut r, &scene);
    let after = r.ray_tree(&scene).unwrap().packed().instances.len();
    assert_eq!(after, before - 1, "a layer no camera draws is left out");
}
