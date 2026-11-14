//! A frame's volumes never outgrow one storage binding (#1672).

use glam::{Mat4, Vec3};

use super::*;

#[test]
fn room_is_the_largest_power_of_two_within_the_binding() {
    assert_eq!(room(128 << 20), (128 << 20) / 16);
    assert_eq!(room((128 << 20) + 5), (128 << 20) / 16, "rounded down");
    assert_eq!(room(0), 0);
    assert_eq!(room(u64::from(u32::MAX)), (1 << 31) / 16);
}

#[test]
fn a_volume_past_the_frame_s_room_takes_the_single_sample() {
    let mut scene = Scene::new();
    scene.probes = rising_probes();
    let (a, b) = (scene.add_entity("a".into()), scene.add_entity("b".into()));
    for id in [a, b] {
        let volume = locker().0;
        scene.world.set_light_probe_proxy_volume(id, Some(volume));
    }
    let at = (
        Mat4::IDENTITY,
        (Vec3::new(-0.3, 0.0, -0.3), Vec3::new(0.3, 2.0, 0.3)),
    );
    let mut volumes = ProxyVolumes::default();
    volumes.begin_frame(&scene);
    // One volume is 21 vec4s; 32 fit (the largest power of two within 40 * 16 bytes).
    let mut frame = volumes.frame(40 * 16);
    assert_eq!(volumes.push(&scene, a, at, &mut frame), 1);
    assert_eq!(volumes.push(&scene, b, at, &mut frame), 0, "no room left");
    assert_eq!(frame.data.len(), HEADER + 4 * COEFFS);
    assert!(volumes.warned, "the console is told");
    let mut roomy = volumes.frame(u64::MAX);
    volumes.push(&scene, a, at, &mut roomy);
    assert_eq!(volumes.push(&scene, b, at, &mut roomy), 22);
}
