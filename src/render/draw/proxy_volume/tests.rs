//! Packing a Light Probe Proxy Volume and when it is sampled again (#1672).

use glam::{Mat4, Vec3};

use super::*;
use crate::components::{ProxyQuality, ProxyResolutionMode};

/// A probe grid over `[-2, 2] x [0, 4] x [-2, 2]` whose light grows with height:
/// SH band 0 is `y`, band 1's y term `-y` (light from above), band 2 is `1`.
fn rising_probes() -> ProbeVolume {
    let mut probes = ProbeVolume::new();
    probes.fill_grid(Vec3::new(-2.0, 0.0, -2.0), Vec3::new(2.0, 4.0, 2.0), 1.0);
    for p in &mut probes.probes {
        let y = p.position.y;
        p.sh.coeffs[0] = [y; 3];
        p.sh.coeffs[1] = [-y; 3];
        p.sh.coeffs[2] = [1.0; 3];
        p.sh.coeffs[4] = [9.0; 3];
    }
    probes
}

/// A 1 x 4 x 1 grid over a 2 m locker standing at the origin.
fn locker() -> (Volume, ProxyGrid) {
    let v = Volume {
        resolution_mode: ProxyResolutionMode::Custom,
        grid_resolution: [1, 4, 1],
        ..Default::default()
    };
    let grid = v.grid((Vec3::new(-0.3, 0.0, -0.3), Vec3::new(0.3, 2.0, 0.3)));
    (v, grid)
}

#[test]
fn a_volume_packs_its_box_its_dims_and_each_point_s_l1_sample() {
    let probes = rising_probes();
    let (v, grid) = locker();
    let model = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
    let data = pack(&v, &grid, model, &probes).unwrap();
    assert_eq!(data.len(), HEADER + 4 * COEFFS);
    let to_box = Mat4::from_cols_array_2d(&[data[0], data[1], data[2], data[3]]);
    let foot = to_box.transform_point3(Vec3::new(0.7, 0.0, -0.3));
    let head = to_box.transform_point3(Vec3::new(1.3, 2.0, 0.3));
    assert!(foot.abs_diff_eq(Vec3::ZERO, 1e-5) && head.abs_diff_eq(Vec3::ONE, 1e-5));
    assert_eq!(data[4].map(f32::to_bits), [1, 4, 1, 0]);
    // Points at cell centres up the locker: 0.25, 0.75, 1.25, 1.75 m.
    let bands0: Vec<f32> = (0..4).map(|p| data[HEADER + p * COEFFS][0]).collect();
    assert_eq!(bands0, [0.25, 0.75, 1.25, 1.75]);
    let first = &data[HEADER..HEADER + COEFFS];
    assert_eq!(first[1][0], -0.25, "band 1 kept at Normal");
    assert_eq!(first[2][0], 1.0);
}

#[test]
fn low_quality_keeps_band_zero_alone() {
    let probes = rising_probes();
    let (mut v, grid) = locker();
    v.quality = ProxyQuality::Low;
    let data = pack(&v, &grid, Mat4::IDENTITY, &probes).unwrap();
    let first = &data[HEADER..HEADER + COEFFS];
    assert_eq!(first[0][0], 0.25);
    assert_eq!([first[1], first[2], first[3]], [[0.0; 4]; 3]);
}

#[test]
fn no_probes_or_a_zero_scale_packs_nothing() {
    let (v, grid) = locker();
    assert!(pack(&v, &grid, Mat4::IDENTITY, &ProbeVolume::new()).is_none());
    let flat = Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0));
    assert!(pack(&v, &grid, flat, &rising_probes()).is_none());
}

#[test]
fn a_volume_is_sampled_again_only_when_something_it_reads_changed() {
    let mut scene = Scene::new();
    scene.probes = rising_probes();
    let id = scene.add_entity("locker".to_string());
    let bare = scene.add_entity("bare".to_string());
    scene
        .world
        .set_light_probe_proxy_volume(id, Some(locker().0));
    let bounds = (Vec3::new(-0.3, 0.0, -0.3), Vec3::new(0.3, 2.0, 0.3));
    let mut volumes = ProxyVolumes::default();
    let at = |x: f32| (Mat4::from_translation(Vec3::X * x), bounds);
    volumes.begin_frame(&scene);
    let mut frame = volumes.frame();
    assert_eq!(
        volumes.push(&scene, bare, at(0.0), &mut frame),
        0,
        "no volume"
    );
    assert_eq!(volumes.push(&scene, id, at(0.0), &mut frame), 1);
    assert_eq!(
        volumes.push(&scene, id, at(0.0), &mut frame),
        22,
        "back to back"
    );
    let sampled = |v: &ProxyVolumes| v.cache[&id].1[HEADER][0];
    // Poison the cache: a still object keeps it, anything it reads refreshes it.
    volumes.cache.get_mut(&id).unwrap().1[HEADER][0] = -1.0;
    volumes.begin_frame(&scene);
    let mut frame = volumes.frame();
    volumes.push(&scene, id, at(0.0), &mut frame);
    assert_eq!(
        sampled(&volumes),
        -1.0,
        "nothing changed: not sampled again"
    );
    volumes.push(&scene, id, at(0.5), &mut frame);
    assert_eq!(sampled(&volumes), 0.25, "moved: sampled again");
    volumes.cache.get_mut(&id).unwrap().1[HEADER][0] = -1.0;
    scene.probes.probes[0].sh.coeffs[0] = [5.0; 3];
    volumes.begin_frame(&scene);
    let mut frame = volumes.frame();
    volumes.push(&scene, id, at(0.5), &mut frame);
    assert_eq!(
        sampled(&volumes),
        0.25,
        "a rebaked probe field: sampled again"
    );
    scene.world.set_light_probe_proxy_volume(id, None);
    volumes.begin_frame(&scene);
    let forgotten = volumes.cache.is_empty() && volumes.frame().probes.is_none();
    assert!(forgotten, "removed: forgotten");
}

#[test]
fn the_fingerprint_sees_any_probe_s_sh_position_or_the_grid() {
    let probes = rising_probes();
    let base = fingerprint(&probes);
    assert_eq!(base, fingerprint(&rising_probes()), "deterministic");
    let mut moved = rising_probes();
    moved.probes[7].position.x += 0.01;
    let mut rebaked = rising_probes();
    rebaked.probes[42].sh.coeffs[8][2] = 3.0;
    let mut regridded = rising_probes();
    regridded.grid.as_mut().unwrap().spacing.y = 2.0;
    for changed in [moved, rebaked, regridded] {
        assert_ne!(fingerprint(&changed), base);
    }
}
