use super::*;

#[test]
fn speed_is_the_reference_over_the_probe() {
    assert_eq!(speed(Some(REFERENCE_MS), Some(REFERENCE_MS)), Some(1.0));
    // Twice as slow as the reference: half the speed.
    let slow = 2.0 * REFERENCE_MS;
    assert_eq!(speed(Some(slow), Some(slow)), Some(0.5));
}

#[test]
fn speed_averages_the_probes_on_either_side_of_the_frame() {
    // Probes of 1x and 3x the reference average to 2x: half the speed.
    let got = speed(Some(REFERENCE_MS), Some(3.0 * REFERENCE_MS));
    assert_eq!(got, Some(0.5));
}

#[test]
fn speed_uses_the_one_probe_that_read() {
    let slow = 4.0 * REFERENCE_MS;
    assert_eq!(speed(Some(slow), None), Some(0.25));
    assert_eq!(speed(None, Some(slow)), Some(0.25));
    assert_eq!(speed(None, None), None);
}

/// On an adapter with timestamps the probe reads a positive time every call;
/// without them there is no probe at all.
#[test]
fn gpu_the_probe_times_its_load_when_the_adapter_has_timestamps() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let Some(probe) = ClockProbe::new(device, queue) else {
        assert!(!device.features().contains(wgpu::Features::TIMESTAMP_QUERY));
        return;
    };
    probe.time(device, queue); // pipeline setup
    for _ in 0..3 {
        let ms = probe.time(device, queue).expect("a timed probe");
        assert!(ms > 0.0 && ms < 1000.0, "{ms}");
    }
}
