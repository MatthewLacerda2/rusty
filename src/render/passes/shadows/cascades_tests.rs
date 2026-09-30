use super::*;

const SIZE: u32 = 1024;
const SUN: Vec3 = Vec3::new(-0.5, -1.0, -0.3);

fn camera_at(position: Vec3, yaw: f32) -> Camera {
    Camera::new(position, yaw, -10.0)
}

fn fit_default(camera: &Camera) -> Vec<Cascade> {
    fit(camera, 16.0 / 9.0, SUN, &ShadowSettings::default(), SIZE)
}

/// The eight corners of the camera frustum between view depths `near` and `far`.
fn slice_corners(camera: &Camera, aspect: f32, near: f32, far: f32) -> Vec<Vec3> {
    let fwd = camera.forward();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let tan_v = (camera.fov.to_radians() * 0.5).tan();
    let mut corners = Vec::new();
    for d in [near, far] {
        for (sx, sy) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            let lateral = right * sx * tan_v * aspect * d + up * sy * tan_v * d;
            corners.push(camera.position + fwd * d + lateral);
        }
    }
    corners
}

fn inside(cascade: &Cascade, p: Vec3) -> bool {
    let ndc = cascade.light_space.project_point3(p);
    ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z)
}

#[test]
fn splits_grow_to_the_shadow_distance() {
    let splits = split_distances(0.1, 100.0, 4);
    assert!(splits.windows(2).all(|w| w[0] < w[1]), "{splits:?}");
    assert!((splits[3] - 100.0).abs() < 1e-3);
    assert!(
        splits[0] < 10.0,
        "the first cascade stays close: {splits:?}"
    );
}

#[test]
fn every_cascade_contains_its_slice_far_from_the_origin() {
    // The #435 bug: nothing past ±30 units of the origin was ever in the shadow map.
    for position in [
        Vec3::ZERO,
        Vec3::new(100.0, 2.0, -40.0),
        Vec3::new(-250.0, 30.0, 0.0),
    ] {
        let camera = camera_at(position, 37.0);
        let cascades = fit_default(&camera);
        assert_eq!(cascades.len(), 4);
        let mut start = camera.near;
        for c in &cascades {
            for p in slice_corners(&camera, 16.0 / 9.0, start, c.split) {
                assert!(
                    inside(c, p),
                    "{p} escapes the cascade ending at {}",
                    c.split
                );
            }
            start = c.split * (1.0 - BLEND_FRACTION);
        }
    }
}

#[test]
fn casters_toward_the_sun_are_captured() {
    let camera = camera_at(Vec3::new(100.0, 2.0, 0.0), 0.0);
    let near = fit_default(&camera)[0];
    let above = camera.position - SUN.normalize() * 150.0;
    assert!(
        inside(&near, above),
        "a caster 150 units up-sun still casts"
    );
}

#[test]
fn near_shadows_are_sharper_than_the_old_single_map() {
    let old_texel = 60.0 / 2048.0;
    let near = fit_default(&camera_at(Vec3::ZERO, 0.0))[0];
    assert!(
        near.texel < old_texel / 1.5,
        "{} vs {old_texel}",
        near.texel
    );
}

#[test]
fn small_moves_keep_every_light_volume_so_the_static_bake_is_reused() {
    let a = fit_default(&camera_at(Vec3::new(10.0, 2.0, 5.0), 20.0));
    let b = fit_default(&camera_at(Vec3::new(10.02, 2.0, 5.01), 20.0));
    assert_eq!(a, b);
}

#[test]
fn turning_never_resizes_a_cascade() {
    let a = fit_default(&camera_at(Vec3::ZERO, 0.0));
    let b = fit_default(&camera_at(Vec3::ZERO, 133.0));
    let texels = |cs: &[Cascade]| cs.iter().map(|c| c.texel).collect::<Vec<_>>();
    assert_eq!(texels(&a), texels(&b));
}

#[test]
fn the_cascade_count_and_distance_follow_the_settings() {
    let camera = camera_at(Vec3::ZERO, 0.0);
    let two = ShadowSettings {
        cascades: 2,
        distance: 40.0,
    };
    let cascades = fit(&camera, 1.5, SUN, &two, SIZE);
    assert_eq!(cascades.len(), 2);
    assert!((cascades[1].split - 40.0).abs() < 1e-3);
    let many = ShadowSettings { cascades: 9, ..two };
    assert_eq!(fit(&camera, 1.5, SUN, &many, SIZE).len(), MAX_CASCADES);
    let straight_down = fit(&camera, 1.5, Vec3::NEG_Y, &two, SIZE);
    assert!(
        straight_down[0].light_space.is_finite(),
        "a noon sun still fits"
    );
}
