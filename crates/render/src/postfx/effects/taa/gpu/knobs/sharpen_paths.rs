use super::super::fixture::{eye, renderer, settle, view};
use super::sharpen::sharpen_scene;
use crate::gpu::ops::CountedQueue as _;
use crate::motion::gpu_fixture::{H, W};
use crate::postfx::effects::taa::sharpen::{run, SharpenTargets};
use crate::postfx::effects::EffectCx;
use crate::postfx::params::PostSource;
use crate::postfx::PostFxContext;
use crate::readback;
use crate::timing::GpuTimer;
use rusty_components::CameraComponent;
use rusty_core::settings::GraphicsSettings;

#[test]
fn gpu_sharpen_through_a_target_matches_writing_the_output() {
    let Some(mut r) = renderer() else { return };
    let (scene, _) = sharpen_scene(0.9);
    let mut v = view(&r);
    let direct = settle(&mut r, &mut v, &scene, &eye(0.0), 4);
    let held = v.post_fx.targets.get::<SharpenTargets>();
    assert!(
        !held.expect("it ran").has_target(),
        "a last sharpen held a target"
    );

    let camera = CameraComponent {
        taa_contrast_adaptive_sharpening: 0.9,
        ..Default::default()
    };
    let cx = EffectCx {
        post: PostSource {
            camera: Some(&camera),
            volume: None,
        },
        graphics: GraphicsSettings::default(),
        full: true,
    };
    let (sky, timer) = (r.sky_panorama(), GpuTimer::disabled());
    let unused = v.color_target_view().unwrap();
    let ctx = PostFxContext {
        depth_view: &v.depth_view,
        skybox: (&sky.view, &sky.sampler),
        output: &unused,
        timer: &timer,
        passes: &r.post_passes,
    };
    let gpu = (&r.device, &r.queue);
    let mut encoder = r.device.create_command_encoder(&Default::default());
    let ended = run(&mut v.post_fx, gpu, &mut encoder, &ctx, &cx, false);
    r.queue.counted_submit(Some(encoder.finish()));
    assert!(!ended, "a sharpen ahead of another pass wrote the output");
    let held = v.post_fx.targets.get::<SharpenTargets>();
    assert!(held.unwrap().has_target(), "not last, it needs its target");

    let ldr = &v.post_fx.ldr.texture;
    let through = readback::read_texture_rgba8(&r.device, &r.queue, ldr, W, H);
    assert_eq!(through, direct, "the two paths sharpened differently");
}
