use super::*;

#[test]
fn reach_grows_with_the_jitter_and_the_size() {
    let reach = |j: Jitter, size| j.reach(size, 1.0);
    assert_eq!(reach(Jitter::new(0.0, 0.0, 0.0), 1.0), (0, 0));
    assert_eq!(reach(Jitter::new(1.0, 0.0, 0.0), 1.0), (1, 1));
    assert_eq!(reach(Jitter::new(1.0, 1.0, 1.0), 1.0), (2, 2));
    assert_eq!(reach(Jitter::new(0.0, 0.0, 0.0), 2.0), (1, 1));
    let most = reach(Jitter::new(1.0, 1.0, 1.0), MAX_SIZE);
    assert_eq!(most, (MAX_REACH, MAX_REACH));
}

#[test]
fn a_turned_stamp_in_a_flat_cell_reaches_more_rows() {
    assert_eq!(Jitter::new(0.0, 0.0, 0.0).reach(1.0, 0.25), (0, 0));
    assert_eq!(Jitter::new(0.0, 0.5, 0.0).reach(1.0, 0.25), (1, 2));
    assert_eq!(Jitter::new(0.0, 0.05, 0.0).reach(1.0, 0.25), (1, 1));
}

#[test]
fn a_turn_in_a_flat_cell_is_rigid_in_the_tile() {
    let (per, cell) = ((4, 16), [0.25, 1.0 / 16.0]);
    for angle in [0.0, 0.3, 1.0, std::f32::consts::FRAC_PI_2] {
        let st = Stamp {
            offset: [0.0; 2],
            turn: (f32::cos(angle), f32::sin(angle)),
            inv_scale: 1.0,
            stretch: stretch(per),
            value: 1.0,
        };
        let o = st.local(0.0, 0.0);
        let col = |x, y| {
            let p = st.local(x, y);
            [p.0 - o.0, p.1 - o.1]
        };
        let (a, b) = (col(1.0, 0.0), col(0.0, 1.0));
        let det = a[0] * b[1] - b[0] * a[1];
        let u = [b[1] / det * cell[0], -a[1] / det * cell[1]];
        let v = [-b[0] / det * cell[0], a[0] / det * cell[1]];
        let len = |p: [f32; 2]| p[0].hypot(p[1]);
        assert!((u[0] * v[0] + u[1] * v[1]).abs() < 1e-7, "{angle}: skewed");
        assert!((len(u) - cell[0]).abs() < 1e-6, "{angle}: {}", len(u));
        assert!((len(v) - cell[1]).abs() < 1e-6, "{angle}: {}", len(v));
    }
}

#[test]
fn the_index_counts_row_major_from_one() {
    assert_eq!(index((0, 0), (2, 2)), 0.25);
    assert_eq!(index((1, 0), (2, 2)), 0.5);
    assert_eq!(index((1, 1), (2, 2)), 1.0);
    assert_eq!(index((0, 1), (2, 4)), 0.375);
}
