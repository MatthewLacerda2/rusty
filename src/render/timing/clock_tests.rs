use super::*;

/// On an adapter with timestamps the probe reads a positive time every call;
/// without them there is no probe at all.
#[test]
fn gpu_the_probe_times_its_load_when_the_adapter_has_timestamps() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let Some(probe) = ClockProbe::new(&renderer) else {
        let features = renderer.device.features();
        assert!(!features.contains(wgpu::Features::TIMESTAMP_QUERY));
        return;
    };
    probe.time(&renderer); // pipeline setup
    for _ in 0..3 {
        let ms = probe.time(&renderer).expect("a timed probe");
        assert!(ms > 0.0 && ms < 1000.0, "{ms}");
    }
}
