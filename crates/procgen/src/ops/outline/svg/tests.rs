//! SVG path data: every command, relative forms, and refusals that name the byte.

use glam::Vec2;

use super::*;

fn points(d: &str) -> Vec<Vec2> {
    parse(d)
        .unwrap()
        .into_iter()
        .flat_map(|s| s.points)
        .collect()
}

fn close(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < 1e-5
}

#[test]
fn lines_take_absolute_relative_and_implicit_forms() {
    let want = [(0.1, 0.1), (0.5, 0.1), (0.5, 0.5), (0.1, 0.5)].map(Vec2::from);
    for d in [
        "M0.1 0.1 L0.5 0.1 L0.5 0.5 L0.1 0.5 Z",
        "m.1,.1 .4,0 0 .4 -.4 0z",
        "M 0.1 0.1 H 0.5 V 0.5 h -0.4 Z",
        "M1e-1 1E-1L5e-1,.1 .5.5 .1.5z",
    ] {
        let p = points(d);
        assert!(p.len() >= 4, "{d}: {p:?}");
        for (a, b) in p.iter().zip(want) {
            assert!(close(*a, b), "{d}: {p:?}");
        }
        assert!(parse(d).unwrap()[0].closed);
    }
}

#[test]
fn curves_end_on_their_endpoints_and_smooth_ones_reflect() {
    for d in [
        "M0 0 C0.2 0 0.4 0.2 0.4 0.4",
        "M0 0 Q0.4 0 0.4 0.4",
        "M0 0 c.2 0 .4 .2 .4 .4 s.4 .2 .4 .4",
        "M0 0 q.4 0 .4 .4 t.4 .4",
    ] {
        let p = points(d);
        assert!(p.len() > 8, "{d} is cut into chords");
        let end = *p.last().unwrap();
        assert!(
            close(end, Vec2::new(0.4, 0.4)) || close(end, Vec2::new(0.8, 0.8)),
            "{d}: {end}"
        );
    }
    // `S` after `C` mirrors the second control point: the joint is smooth.
    let p = points("M0 0 C0 0.2 0.2 0.2 0.2 0 S0.4 -0.2 0.4 0");
    let joint = p
        .iter()
        .position(|q| close(*q, Vec2::new(0.2, 0.0)))
        .unwrap();
    let (before, after) = (p[joint] - p[joint - 1], p[joint + 1] - p[joint]);
    assert!(before.normalize().dot(after.normalize()) > 0.99);
}

#[test]
fn an_arc_stays_on_its_circle_and_flags_pick_the_side() {
    // A half circle of radius 0.25 about (0.5, 0.5), both sweeps.
    for d in [
        "M0.25 0.5 A0.25 0.25 0 0 1 0.75 0.5",
        "M0.25 0.5 a.25.25 0 00.5 0",
    ] {
        let p = points(d);
        assert!(p.len() > 16);
        for q in &p {
            assert!(
                (q.distance(Vec2::splat(0.5)) - 0.25).abs() < 1e-4,
                "{d}: {q}"
            );
        }
        let side = p[p.len() / 2].y - 0.5;
        assert!(side.abs() > 0.2);
    }
    let up = points("M0.25 0.5 A0.25 0.25 0 0 1 0.75 0.5");
    let down = points("M0.25 0.5 A0.25 0.25 0 0 0 0.75 0.5");
    assert!(
        up[up.len() / 2].y * down[down.len() / 2].y < 0.3,
        "opposite halves"
    );
    // Radii too small to reach are scaled up; a zero radius is a line.
    assert!(close(
        *points("M0 0 A0.01 0.01 0 0 1 0.5 0").last().unwrap(),
        Vec2::new(0.5, 0.0)
    ));
    assert_eq!(points("M0 0 A0 0.1 0 0 1 0.5 0").len(), 2);
}

#[test]
fn a_malformed_path_names_the_byte() {
    for (d, at, what) in [
        ("L0 0", 0, "starts with `M`"),
        ("M0 0 L0.5", 9, "expected a number"),
        ("M0 0 X1 1", 5, "unknown command `X`"),
        ("M0 0 L0.5 0.5 x", 14, "unknown command `x`"),
        ("M0 0 A.1 .1 0 2 1 .5 .5", 14, "arc flag"),
        ("0 0", 0, "starts with `M`"),
        ("M0 0 L9 0", 9, "past ±4 tiles"),
        ("M0 0 L1e40 0", 6, "finite number"),
        ("M0 0 Z 1", 7, "expected a command"),
    ] {
        let e = parse(d).expect_err(d);
        assert!(
            e.starts_with(&format!("at byte {at}: ")) && e.contains(what),
            "{d}: {e}"
        );
    }
}

#[test]
fn a_pathological_path_is_refused_without_running_away() {
    let long = "M0 0".to_string() + &" L0.1 0.1".repeat(MAX_BYTES / 8);
    assert!(parse(&long).unwrap_err().contains("bytes"));
    // Many huge arcs: each is capped in chords, and the whole in points.
    let arcs = "M0 0".to_string() + &" A4 4 0 1 1 0.001 0 A4 4 0 1 1 0 0".repeat(400);
    let e = parse(&arcs).unwrap_err();
    assert!(e.contains("points") || e.contains("tiles"), "{e}");
    assert!(parse("").unwrap().is_empty());
    assert!(
        parse("M0 0").unwrap().is_empty(),
        "a lone move draws nothing"
    );
}
