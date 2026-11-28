use glam::Vec3;

use super::ProbeField;
use crate::lighting::probe::ProbeVolume;

/// Probes along x from `from`, `n` of them a metre apart, probe `k`'s DC term `dc + k`.
fn row(from: f32, n: usize, dc: f32) -> ProbeVolume {
    let mut v = ProbeVolume::new();
    for k in 0..n {
        let i = v.add_probe(Vec3::new(from + k as f32, 0.3 * (k % 2) as f32, 0.0));
        v.probes_mut()[i].sh.coeffs[0] = [dc + k as f32, 1.0, 0.0];
    }
    v
}

fn dc(field: ProbeField<'_>, at: Vec3) -> f32 {
    field.sample(at).unwrap().coeffs[0][0]
}

#[test]
fn one_set_samples_as_the_set_alone_does() {
    let a = row(0.0, 40, 0.0);
    let sets = [(0, &a)];
    for x in [-3.0, 0.0, 0.25, 7.7, 39.0, 80.0] {
        let at = Vec3::new(x, 0.1, 0.2);
        assert_eq!(ProbeField::new(&sets).sample(at), a.sample(at), "{x}");
    }
}

#[test]
fn a_point_between_two_sets_is_lit_by_both() {
    let (a, b) = (row(0.0, 3, 0.0), row(4.0, 3, 100.0));
    let sets = [(0, &a), (1, &b)];
    let mid = dc(ProbeField::new(&sets), Vec3::new(3.0, 0.0, 0.0));
    assert!(
        mid > 2.0 && mid < 100.0,
        "{mid}: a blend of a's (≤ 2) and b's (≥ 100)"
    );
}

#[test]
fn a_set_out_of_reach_changes_no_bit() {
    let (a, b) = (row(0.0, 40, 0.0), row(100.0, 5, 100.0));
    let (both, alone) = ([(0, &a), (1, &b)], [(0, &a)]);
    let field = ProbeField::new(&both);
    let far = Vec3::new(2.0, 0.0, 1.0);
    let reach = field.reach_without(far, |h| h != 1);
    let gap = b.bounds().unwrap().0.distance(far);
    assert!(reach < gap, "b is beyond the far point's reach");
    let alone = ProbeField::new(&alone);
    assert_eq!(field.sample(far), alone.sample(far), "bit-identical");
    let near = Vec3::new(98.0, 0.0, 0.0);
    assert!(field.reach_without(near, |h| h != 1) > b.bounds().unwrap().0.distance(near));
    assert_ne!(
        field.sample(near),
        alone.sample(near),
        "a near point sees b"
    );
}

#[test]
fn a_point_in_a_grid_reads_the_grid_and_no_set_is_empty_field() {
    let mut a = ProbeVolume::new();
    a.fill_grid(Vec3::ZERO, Vec3::splat(2.0), 1.0);
    let b = row(1.0, 3, 100.0);
    let sets = [(0, &a), (1, &b)];
    let at = Vec3::splat(1.0);
    assert_eq!(ProbeField::new(&sets).sample(at), a.sample(at));
    let empty = ProbeVolume::new();
    let none = [(0, &empty)];
    assert!(ProbeField::new(&none).sample(at).is_none());
    assert_eq!(
        ProbeField::new(&none).reach_without(at, |_| true),
        f32::INFINITY
    );
}
