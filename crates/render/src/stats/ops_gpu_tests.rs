use glam::{Quat, Vec3};

use super::OpCounts;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, create_live_light, Primitive};
use rusty_scene::{Camera, Scene};

const STILL: OpCounts = OpCounts {
    render_passes: 7,
    compute_passes: 0,
    submits: 3,
    bind_groups_created: 3,
    buffer_writes: 18,
};

const STILL_CULLING: OpCounts = OpCounts {
    render_passes: 8,
    compute_passes: 2,
    submits: 3,
    bind_groups_created: 3,
    buffer_writes: 21,
};

const STILL_CULLING_TIMED: OpCounts = OpCounts {
    compute_passes: 3,
    ..STILL_CULLING
};

fn late_prepass(renderer: &Renderer, want: OpCounts) -> OpCounts {
    let primes = renderer.depth_priming && renderer.priming_recommended;
    OpCounts {
        render_passes: want.render_passes + u32::from(primes),
        ..want
    }
}

const W: u32 = 64;
const H: u32 = 48;

fn still_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(floor).unwrap().scale = Vec3::splat(4.0);
    let kinds = [Primitive::Box, Primitive::Sphere, Primitive::Cylinder];
    for (n, kind) in kinds.into_iter().enumerate() {
        let id = create_entity(&mut scene, &format!("Prop_{n}"), Some(kind));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(n as f32 - 1.0, 0.5, 0.0);
    }
    scene
}

fn frames(renderer: &mut Renderer, scene: &Scene, n: usize) -> (Vec<OpCounts>, u32) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 3.0, 5.0), -90.0, -25.0);
    let ops = (0..n)
        .map(|_| {
            renderer.render(&mut view, scene, &cam, &out, false);
            renderer.frame_counters.ops
        })
        .collect();
    (ops, renderer.frame_counters.hiz_tested)
}

#[test]
fn gpu_a_still_frame_makes_the_calls_the_one_before_made() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = still_scene();
    let (counts, tested) = frames(&mut renderer, &scene, 4);
    assert_eq!(counts[1..], [STILL; 3], "every still frame: {counts:#?}");
    assert_eq!(tested, 0, "a seeded preset runs no Hi-Z stage");
    assert!(counts[0].bind_groups_created > STILL.bind_groups_created);
}

#[test]
fn gpu_a_still_frame_with_the_gpu_cull_on_makes_its_stage_s_calls() {
    for (timed, want) in [(false, STILL_CULLING), (true, STILL_CULLING_TIMED)] {
        let Some(mut renderer) = crate::test_gpu::culling::culling_or_skip(W, H) else {
            return;
        };
        renderer.gpu_timer.split_override.set(Some(timed));
        let scene = still_scene();
        let (counts, tested) = frames(&mut renderer, &scene, 4);
        let want = late_prepass(&renderer, want);
        assert_eq!(counts[1..], [want; 3], "timed {timed}: {counts:#?}");
        let late = renderer.frame_counters.hiz_late;
        assert_eq!(
            late, 0,
            "a held view's late set is empty: the prepass adds nothing"
        );
        assert!(tested > 0, "the floor and the props are tested");
    }
}

#[test]
fn gpu_each_flush_shape_adds_its_submits() {
    use crate::EarlyFlushes;
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = still_scene();
    for (flushes, submits) in [
        (EarlyFlushes::None, 1),
        (EarlyFlushes::Occlusion, 2),
        (EarlyFlushes::OcclusionScene, 3),
    ] {
        renderer.switches.set_flushes(flushes);
        let (counts, _) = frames(&mut renderer, &scene, 2);
        let want = OpCounts { submits, ..STILL };
        assert_eq!(counts[1], want, "{flushes}: only the submits move");
    }
}
