//! The volume's grid: its box, its resolution and its points (#1672).

use super::*;

const UNIT: (Vec3, Vec3) = (Vec3::splat(-0.5), Vec3::splat(0.5));

#[test]
fn a_resolution_is_a_power_of_two_from_one_to_thirty_two() {
    assert_eq!(resolution(0), 1);
    assert_eq!(resolution(1), 1);
    assert_eq!(resolution(3), 4);
    assert_eq!(resolution(17), 32);
    assert_eq!(resolution(u32::MAX), MAX_RESOLUTION);
}

#[test]
fn automatic_bounds_span_the_mesh_at_the_density() {
    // A 2 m locker, 0.6 m wide: two points per metre.
    let v = LightProbeProxyVolumeComponent {
        probe_density: 2.0,
        ..Default::default()
    };
    let locker = (Vec3::new(-0.3, 0.0, -0.3), Vec3::new(0.3, 2.0, 0.3));
    let g = v.grid(locker);
    assert_eq!(g.dims, [1, 4, 1], "1.2 rounds to 1; 4 along the height");
    assert_eq!(g.min, locker.0);
    assert_eq!(g.size, locker.1 - locker.0);
    assert_eq!(g.len(), 4);
    let ys: Vec<f32> = g.points().map(|p| p.y).collect();
    assert_eq!(ys, [0.25, 0.75, 1.25, 1.75], "cell centres, bottom first");
}

#[test]
fn custom_bounds_and_resolution_ignore_the_mesh() {
    let v = LightProbeProxyVolumeComponent {
        bounds_mode: ProxyBoundsMode::Custom,
        size: Vec3::new(2.0, 4.0, 2.0),
        origin: Vec3::new(0.0, 2.0, 0.0),
        resolution_mode: ProxyResolutionMode::Custom,
        grid_resolution: [3, 64, 0],
        ..Default::default()
    };
    let g = v.grid(UNIT);
    assert_eq!(g.dims, [4, 32, 1]);
    assert_eq!(g.min, Vec3::new(-1.0, 0.0, -1.0));
    let first = g.points().next().unwrap();
    assert_eq!(first, Vec3::new(-0.75, 0.0625, 0.0));
}

#[test]
fn points_run_x_fastest_then_y_then_z() {
    let v = LightProbeProxyVolumeComponent {
        resolution_mode: ProxyResolutionMode::Custom,
        grid_resolution: [2, 2, 2],
        ..Default::default()
    };
    let g = v.grid(UNIT);
    let p: Vec<Vec3> = g.points().collect();
    assert_eq!(p[1] - p[0], Vec3::new(0.5, 0.0, 0.0));
    assert_eq!(p[2] - p[0], Vec3::new(0.0, 0.5, 0.0));
    assert_eq!(p[4] - p[0], Vec3::new(0.0, 0.0, 0.5));
    assert_eq!(p[7], g.point(1, 1, 1));
}

#[test]
fn a_flat_or_malformed_box_still_resolves() {
    let plane = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0));
    let g = LightProbeProxyVolumeComponent::default().grid(plane);
    assert_eq!(g.dims, [2, 1, 2], "a zero-height mesh gets one layer");
    assert!(g.size.y > 0.0);
    let bad = LightProbeProxyVolumeComponent {
        bounds_mode: ProxyBoundsMode::Custom,
        size: Vec3::new(f32::NAN, 1.0, 1.0),
        origin: Vec3::splat(f32::INFINITY),
        probe_density: f32::NAN,
        ..Default::default()
    };
    let g = bad.grid(UNIT);
    assert_eq!(
        (g.min, g.size, g.dims),
        (Vec3::splat(-0.5), Vec3::ONE, [1, 1, 1])
    );
}

#[test]
fn a_saved_volume_round_trips_by_name() {
    let v = LightProbeProxyVolumeComponent {
        quality: ProxyQuality::Low,
        bounds_mode: ProxyBoundsMode::Custom,
        ..Default::default()
    };
    let text = serde_json::to_string(&v).unwrap();
    assert!(
        text.contains("\"Low\"") && text.contains("\"Custom\""),
        "{text}"
    );
    let back: LightProbeProxyVolumeComponent = serde_json::from_str(&text).unwrap();
    assert_eq!(back, v);
    let empty: LightProbeProxyVolumeComponent = serde_json::from_str("{}").unwrap();
    assert_eq!(empty, LightProbeProxyVolumeComponent::default());
}
