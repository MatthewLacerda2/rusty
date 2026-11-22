use super::*;

#[test]
fn fill_grid_counts_and_positions() {
    let mut vol = ProbeVolume::new();
    vol.fill_grid(Vec3::ZERO, Vec3::new(2.0, 0.0, 2.0), 1.0);
    assert_eq!(vol.probes.len(), 9);
    let grid = vol.grid.unwrap();
    assert_eq!(grid.counts, [3, 1, 3]);
    assert_eq!(vol.probes[0].position, Vec3::ZERO);
}

#[test]
fn add_move_remove_probe() {
    let mut vol = ProbeVolume::new();
    let i = vol.add_probe(Vec3::X);
    assert_eq!(vol.probes.len(), 1);
    vol.move_probe(i, Vec3::Y);
    assert_eq!(vol.probes[i].position, Vec3::Y);
    vol.remove_probe(i);
    assert!(vol.is_empty());
}

#[test]
fn sample_none_when_empty() {
    let vol = ProbeVolume::new();
    assert!(vol.sample(Vec3::ZERO).is_none());
}

#[test]
fn trilinear_blends_between_probes() {
    let mut vol = ProbeVolume::new();
    vol.fill_grid(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0);
    vol.probes[0].sh.coeffs[0] = [0.0, 0.0, 0.0];
    vol.probes[1].sh.coeffs[0] = [1.0, 0.0, 0.0];
    let mid = vol.sample(Vec3::new(0.5, 0.0, 0.0)).unwrap();
    assert!((mid.coeffs[0][0] - 0.5).abs() < 1.0e-5);
}

fn pair() -> ProbeVolume {
    let mut vol = ProbeVolume::new();
    vol.add_probe(Vec3::ZERO);
    vol.add_probe(Vec3::X * 2.0);
    vol.probes[1].sh.coeffs[0] = [1.0, 0.0, 0.0];
    vol
}

#[test]
fn free_probes_blend_by_distance() {
    let vol = pair();
    let at = |x: f32| vol.sample(Vec3::new(x, 0.0, 0.0)).unwrap().coeffs[0][0];
    assert_eq!(at(0.0), 0.0);
    assert_eq!(at(2.0), 1.0);
    assert!((at(1.0) - 0.5).abs() < 1.0e-5);
    let near_first = at(0.5);
    assert!(near_first > 0.0 && near_first < 0.5, "{near_first}");
}

#[test]
fn a_far_probe_is_left_out_of_the_blend() {
    let mut vol = pair();
    for k in 0..BLEND_PROBES {
        vol.add_probe(Vec3::new(1.0, 0.0, 0.1 * k as f32));
    }
    let far = vol.add_probe(Vec3::splat(100.0));
    vol.probes[far].sh.coeffs[0] = [1000.0, 0.0, 0.0];
    let s = vol.sample(Vec3::new(1.0, 0.0, 0.05)).unwrap();
    assert!(s.coeffs[0][0] < 1.0, "{}", s.coeffs[0][0]);
}

#[test]
fn replace_placed_keeps_manual_probes() {
    let mut vol = ProbeVolume::new();
    vol.fill_grid(Vec3::ZERO, Vec3::X, 1.0);
    (0..2).for_each(|i| vol.set_manual(i, true));
    vol.replace_placed([Vec3::Y]);
    vol.replace_placed([Vec3::Z, Vec3::Y * 2.0]);
    let at: Vec<Vec3> = vol.probes.iter().map(|p| p.position).collect();
    assert_eq!(at, [Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::Y * 2.0]);
    assert!(vol.grid.is_some(), "a manual grid stays on the second bake");
    assert!(!vol.probes[2].manual);
    vol.set_manual(1, false);
    vol.replace_placed([]);
    assert!(
        vol.grid.is_none(),
        "a grid probe was placed, so the grid goes"
    );
}

#[test]
fn the_manual_flag_round_trips_and_defaults_to_placed() {
    let mut probe = Probe::new(Vec3::X);
    let placed = serde_json::to_string(&probe).unwrap();
    assert!(!placed.contains("manual"), "{placed}");
    probe.manual = true;
    let text = serde_json::to_string(&probe).unwrap();
    assert!(serde_json::from_str::<Probe>(&text).unwrap().manual);
    assert!(!serde_json::from_str::<Probe>(&placed).unwrap().manual);
    let old = r#"{"position":[0,0,0],"box_min":[-1,-1,-1],"box_max":[1,1,1]}"#;
    let reflection: crate::ReflectionProbe = serde_json::from_str(old).unwrap();
    assert!(
        !reflection.manual,
        "an old reflection probe loads as placed"
    );
}

#[test]
fn a_moved_probe_is_sampled_where_it_now_is() {
    let mut vol = ProbeVolume::new();
    for i in 0..64 {
        vol.add_probe(Vec3::new((i % 8) as f32, 0.0, (i / 8) as f32) * 4.0);
    }
    vol.probes[5].sh.coeffs[0] = [1.0; 3];
    let at = Vec3::new(100.0, 0.0, 100.0);
    assert_ne!(vol.sample(at).unwrap().coeffs[0], [1.0; 3]);
    vol.move_probe(5, at);
    assert_eq!(vol.sample(at).unwrap().coeffs[0], [1.0; 3]);
}
