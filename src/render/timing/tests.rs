use super::{GpuPass, GpuTimer, GpuTimes};

#[test]
fn ticks_sum_per_pass_kind_in_frame_order() {
    // Two shadow sweeps and a forward pass, back to back, at 1 ns per tick.
    let ticks = [0, 1_000_000, 1_000_000, 1_500_000, 2_000_000, 4_000_000];
    let passes = [GpuPass::Shadows, GpuPass::Shadows, GpuPass::Forward];
    let times = GpuTimes::from_ticks(&ticks, &passes, 1.0);
    assert_eq!(
        times.passes,
        vec![(GpuPass::Shadows, 1.5), (GpuPass::Forward, 2.0)]
    );
    assert_eq!(times.total_ms(), 3.5);
}

#[test]
fn an_overlapping_pass_is_charged_only_past_the_work_ahead_of_it() {
    // Forward runs 0..10; post-FX starts its vertices at 2 but cannot finish its
    // fragments before forward's, ending at 11: it costs 1, not 9, and the frame 11.
    let ticks = [0, 10, 2, 11, 12, 13];
    let passes = [GpuPass::Forward, GpuPass::PostFx, GpuPass::Ui];
    let times = GpuTimes::from_ticks(&ticks, &passes, 1e6);
    let want = [
        (GpuPass::Forward, 10.0),
        (GpuPass::PostFx, 1.0),
        (GpuPass::Ui, 1.0),
    ];
    assert_eq!(times.passes, want);
    assert_eq!(times.total_ms(), 12.0);
}

#[test]
fn the_tick_period_scales_and_a_pass_ending_early_counts_zero() {
    // 10 ticks of 0.1 ms; the SSAO pass ends inside the UI pass queued before it.
    let times = GpuTimes::from_ticks(&[10, 20, 12, 18], &[GpuPass::Ui, GpuPass::Ssao], 1e5);
    assert_eq!(times.passes, vec![(GpuPass::Ssao, 0.0), (GpuPass::Ui, 1.0)]);
}

#[test]
fn pass_names_are_stable_and_unique() {
    let names: Vec<_> = GpuPass::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(
        names,
        [
            "shadows",
            "ssao",
            "forward",
            "transparent",
            "ribbons",
            "particles",
            "post_fx",
            "ui"
        ]
    );
    // `from_ticks` indexes by discriminant: ALL must list the variants in order.
    for (i, pass) in GpuPass::ALL.iter().enumerate() {
        assert_eq!(*pass as usize, i);
    }
}

#[test]
fn a_disabled_timer_hands_out_no_writes() {
    let timer = GpuTimer::disabled();
    timer.begin_frame();
    assert!(timer.queries.is_none());
    assert!(timer.writes(GpuPass::Forward).is_none());
}

/// On an adapter with timestamps (Metal, most Vulkan GPUs) a rendered frame comes
/// back timed; without them (lavapipe) it never does — absent, not zero.
#[test]
fn gpu_a_rendered_frame_reports_its_passes_when_the_adapter_has_timestamps() {
    use crate::render::{test_gpu, RenderView, OFFSCREEN_FORMAT};
    use crate::scene::authoring::{create_entity, Primitive};
    let Some(mut renderer) = test_gpu::headless_or_skip(64, 64) else {
        return;
    };
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 64, 64, 2);
    let mut scene = crate::scene::Scene::new();
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let camera = crate::scene::Camera::new(glam::Vec3::new(0.0, 1.0, 5.0), -90.0, 0.0);
    let target = view.color_target_view().expect("offscreen view");
    renderer.render(&mut view, &scene, &camera, &target, false);
    renderer.wait_idle();
    let times = renderer.take_gpu_times();
    if renderer.gpu_timer.queries.is_none() {
        assert_eq!(times, None);
        return;
    }
    let times = times.expect("a waited-for frame is resolved");
    let kinds: Vec<_> = times.passes.iter().map(|(p, _)| *p).collect();
    assert!(kinds.contains(&GpuPass::Shadows), "{kinds:?}");
    assert!(kinds.contains(&GpuPass::Forward), "{kinds:?}");
    assert!(kinds.contains(&GpuPass::PostFx), "{kinds:?}");
    assert!(times.total_ms() > 0.0, "{times:?}");
    // Taken once: the same frame is never reported twice.
    assert_eq!(renderer.take_gpu_times(), None);
}
