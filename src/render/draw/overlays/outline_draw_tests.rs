//! The outline draws from the forward pass's own draw (#1175): for a mirrored child
//! under a moved parent, a posed skin and a LOD-switched group, each outline's mesh,
//! uniform slot (its per-draw uniform and bone palette), instance (its world matrix)
//! and winding twin are the forward draw's. And with no pivot to grow about, a pose
//! far from its entity's origin is ringed evenly (#1165). Skips with no adapter.

use glam::{Mat4, Vec3};

use super::outline_follow_tests::{lod_switched, posed_sphere};
use super::outline_tests::{frame, outline_stats, selected_child, RES};
use crate::render::draw::batch::FrameDraws;
use crate::render::gpu::pipelines::winding::is_mirrored;
use crate::render::lod::LodSelection;
use crate::render::{Frustum, MeshId, Renderer};
use crate::scene::{Camera, Scene};

/// The forward draws of `scene` by the camera `frame` looks through at `centre`.
fn forward_draws(r: &mut Renderer, scene: &Scene, centre: Vec3) -> FrameDraws {
    frame(r, scene, centre); // makes the meshes resident
    let cam = Camera::new(centre + Vec3::Z * 3.0, -90.0, 0.0);
    let frustum = Frustum::from_view_proj(cam.build_view_projection(1.0));
    let lod = LodSelection::for_camera(scene, &cam);
    r.precreate_solid_resources(scene, &cam, &frustum, &lod)
        .draws
}

/// `draws` outlines exactly `ids`, each from the forward draw that drew it; returns
/// each outline's bone palette base, in `ids` order.
fn assert_outlines_forward(scene: &Scene, draws: &FrameDraws, ids: &[u32]) -> Vec<u32> {
    assert_eq!(
        draws.outline.len(),
        ids.len(),
        "one outline per ringed solid"
    );
    let world_of = |i: u32| Mat4::from_cols_array(&draws.instances[i as usize].model_matrix);
    ids.iter()
        .map(|&id| {
            let world = scene.world.local_to_world(id);
            let outline = draws
                .outline
                .iter()
                .find(|o| world_of(o.instance).abs_diff_eq(world, 1e-5))
                .unwrap_or_else(|| panic!("{id}: no outline at its world matrix"));
            let forward = draws
                .batches()
                .find(|b| b.instances.contains(&outline.instance))
                .expect("the outline's instance is a forward draw's");
            let mesh = MeshId::from_mesh(&scene.world.mesh(id).unwrap());
            assert_eq!(
                (&outline.mesh, &forward.key.mesh),
                (&mesh, &mesh),
                "{id}: mesh"
            );
            assert_eq!(outline.uniform_slot, forward.uniform_slot, "{id}: uniform");
            assert_eq!(outline.num_indices, forward.num_indices, "{id}: indices");
            assert_eq!(outline.mirrored, is_mirrored(world), "{id}: winding twin");
            forward.key.uniform().bone_base
        })
        .collect()
}

#[test]
fn gpu_the_outline_draws_from_the_forward_draw() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    for mirrored in [false, true] {
        let (scene, centre) = selected_child(mirrored);
        let child = scene.selected_entity_id.unwrap();
        let draws = forward_draws(&mut r, &scene, centre);
        assert_outlines_forward(&scene, &draws, &[child]);
    }

    let scene = posed_sphere(Vec3::X * 0.4);
    let soldier = scene.selected_entity_id.unwrap();
    let draws = forward_draws(&mut r, &scene, Vec3::ZERO);
    let bases = assert_outlines_forward(&scene, &draws, &[soldier]);
    assert_ne!(bases, [0], "a posed skin's outline binds its palette");

    // LOD1 shows: selecting the group rings its two renderers, not LOD0.
    let (mut scene, [group, _, body, hat]) = lod_switched();
    scene.selected_entity_id = Some(group);
    let draws = forward_draws(&mut r, &scene, Vec3::ZERO);
    assert_outlines_forward(&scene, &draws, &[body, hat]);
}

#[test]
fn gpu_a_root_moved_pose_is_ringed_evenly() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    // The clip carried the root 3 m from the entity's origin, unbaked (#1165).
    let pose = Vec3::X * 3.0;
    let (count, centroid, filled) = outline_stats(&frame(&mut r, &posed_sphere(pose), pose));
    eprintln!("[outline, root moved] {count} px, centroid {centroid:?}");
    assert!(count > 20, "the pose shows an outline");
    assert!(!filled, "the outline is a silhouette, not a fill");
    let off = centroid[0] - RES as f32 / 2.0;
    assert!(
        off.abs() < 1.0,
        "the ring is even on both sides, {off} px off"
    );
}
