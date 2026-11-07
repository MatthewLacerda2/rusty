//! src/render/scene/gpu_tests.rs — a renderer that kept its objects through a run
//! of edits draws what a renderer built fresh on the final state draws (#1706):
//! moving, skinned (posed every frame, unstamped), static, transparent and
//! lightmap-entried solids, with material, layer, activity and override edits, a
//! two-sided, a Cutout and a mirrored static caster: their shadows included. And a
//! move uploads the moved object's record alone (`render_upload_bytes`), and a
//! rebuild none.

use glam::{Mat4, Vec3};

use super::fixture::{frame, yard, RES};
use crate::asset::{JointTransform, SkinData};
use crate::components::mesh::primitives::generate_sphere;
use crate::components::{MaterialComponent, MeshComponent, RenderMode, ShadowCasting};
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::Scene;

/// A one-joint skinned sphere at `at`, posed by its palette.
fn skinned(scene: &mut Scene, at: Vec3) -> u32 {
    let (vertices, indices) = generate_sphere(0.4, 8, 12);
    let skin = SkinData {
        inverse_bind: vec![Mat4::IDENTITY],
        bind_global: vec![Mat4::IDENTITY],
        local_bind: vec![JointTransform::default()],
        parents: vec![None],
        joint_nodes: vec![0],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["root".into()],
    };
    let mesh = MeshComponent {
        bind_palette: skin.bind_palette(),
        skin: Some(skin),
        ..MeshComponent::new("SkinnedSphere", vertices, indices)
    };
    let id = scene.add_entity("Soldier".to_string());
    scene.world.set_mesh(id, Some(mesh));
    scene.world.transform_mut(id).unwrap().position = at;
    id
}

/// Edit `scene` as frame `step` of a run.
fn step(scene: &mut Scene, ids: &[u32], soldier: u32, step: u32) {
    let t = step as f32;
    scene.world.transform_mut(ids[0]).unwrap().position.y = 0.3 * t; // a mover
    let pose = Mat4::from_translation(Vec3::new(0.0, 0.2 * t, 0.0));
    scene.world.set_pose_palette(soldier, vec![pose]); // animation's unstamped write
    match step {
        1 => {
            scene.world.set_static(ids[1], true);
            scene.lightmaps.insert(LightmapEntry {
                entity: ids[1],
                page: 0,
                scale_offset: [1.0, 1.0, 0.0, 0.0],
            });
        }
        2 => {
            let glass = MaterialComponent::new("glass".to_string());
            scene.world.set_material(ids[2], Some(glass));
            scene.materials.get_mut("red").unwrap().base_color = [0.1, 0.9, 0.1];
        }
        3 => {
            scene.world.set_active(ids[3], false);
            scene.world.set_layer(ids[4], 5);
        }
        4 => scene.shader_overrides.set(ids[5], "tint".into(), vec![0.5]),
        5 => scene.world.mesh_mut(ids[1]).unwrap().cast_shadows = ShadowCasting::TwoSided,
        6 => {
            let red = scene.materials.get_mut("red").unwrap();
            (red.render_mode, red.alpha_cutoff) = (RenderMode::Cutout, 0.5);
            scene.world.transform_mut(ids[1]).unwrap().scale.x = -1.0; // mirrored
        }
        _ => {}
    }
}

#[test]
fn gpu_a_kept_scene_draws_what_a_fresh_renderer_draws_after_every_edit() {
    let Some((mut kept, pair)) = crate::render::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    let ids = scene.world.ids_with_mesh();
    let soldier = skinned(&mut scene, Vec3::new(1.0, 0.0, -2.0));
    frame(&mut kept, &scene);
    let mut shown = std::collections::HashSet::new();
    for n in 1..=7 {
        step(&mut scene, &ids, soldier, n);
        // Its static bakes re-baked now, as a fresh renderer's are: the two then
        // differ by nothing (whether a bake goes stale is `equivalence_tests`').
        kept.shadow_renderer.invalidate_static_cache();
        let (pixels, counters) = frame(&mut kept, &scene);
        let mut fresh = pair.renderer(RES, RES);
        let (expected, fresh_counters) = frame(&mut fresh, &scene);
        assert_eq!(
            counters.visible_entities, fresh_counters.visible_entities,
            "step {n}"
        );
        let draws = |c: crate::render::RenderCounters| (c.draw_calls, c.shadow_draws);
        assert_eq!(draws(counters), draws(fresh_counters), "step {n}");
        let differing = pixels
            .chunks(4)
            .zip(expected.chunks(4))
            .filter(|(a, b)| a != b);
        assert_eq!(
            differing.count(),
            0,
            "step {n}: the kept scene drew other pixels"
        );
        shown.extend(pixels.chunks(4).map(<[u8]>::to_vec));
    }
    assert!(shown.len() > 8, "the run shows the solids");
}

#[test]
fn gpu_a_move_uploads_the_moved_record_alone_and_a_rebuild_none() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    frame(&mut r, &scene);
    let idle = frame(&mut r, &scene).1.render_upload_bytes;
    let mover = scene.world.ids_with_mesh()[0];
    scene.world.transform_mut(mover).unwrap().position.y += 0.01;
    let moved = frame(&mut r, &scene).1.render_upload_bytes;
    let record = std::mem::size_of::<crate::render::gpu::uniforms::ObjectData>();
    assert_eq!(
        moved - idle,
        record as u64,
        "one object record, none per static object"
    );
    // A rebuild re-derives every object; each keeps its slot, so none uploads.
    r.retained.touch();
    let rebuilt = frame(&mut r, &scene).1;
    assert_eq!(rebuilt.render_extracted, 6, "the six props, re-derived");
    assert_eq!(rebuilt.render_upload_bytes, idle, "no record moved");
}
