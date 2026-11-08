use super::*;

const DT: f32 = 1.0 / 60.0;
const WHITE: [f32; 4] = [1.0; 4];

/// A one-tick line survives until the next tick starts, then goes.
#[test]
fn a_one_tick_line_lasts_until_the_next_tick() {
    let mut lines = DebugLines::default();
    lines.push(Vec3::ZERO, Vec3::X, WHITE, 1);
    assert_eq!(lines.lines().len(), 1);
    lines.tick();
    assert!(lines.is_empty());
}

/// A second at 60 Hz is sixty ticks: drawn through the sixtieth, gone at the next.
#[test]
fn a_duration_counts_ticks() {
    let mut lines = DebugLines::default();
    lines.push(Vec3::ZERO, Vec3::X, WHITE, duration_ticks(1.0, DT));
    for _ in 0..59 {
        lines.tick();
    }
    assert_eq!(lines.lines().len(), 1, "still drawn after 59 ticks");
    lines.tick();
    assert!(lines.is_empty(), "gone at the 60th");
}

#[test]
fn durations_round_up_to_whole_ticks_and_never_below_one() {
    assert_eq!(duration_ticks(0.0, DT), 1);
    assert_eq!(duration_ticks(-3.0, DT), 1);
    assert_eq!(duration_ticks(f32::NAN, DT), 1);
    assert_eq!(duration_ticks(f32::INFINITY, DT), 1);
    assert_eq!(duration_ticks(1.0, DT), 60);
    assert_eq!(duration_ticks(0.01, DT), 1);
    assert_eq!(duration_ticks(0.02, DT), 2);
    assert_eq!(duration_ticks(1e30, DT), u32::MAX);
}

/// A NaN end draws nothing, and the store keeps its newest lines past the cap.
#[test]
fn bad_lines_drop_and_the_store_is_bounded() {
    let mut lines = DebugLines::default();
    lines.push(Vec3::NAN, Vec3::X, WHITE, 1);
    lines.push(Vec3::ZERO, Vec3::X, [f32::NAN; 4], 1);
    assert!(lines.is_empty());
    for i in 0..=MAX_DEBUG_LINES {
        lines.push(Vec3::ZERO, Vec3::splat(i as f32), WHITE, 1);
    }
    assert_eq!(lines.lines().len(), MAX_DEBUG_LINES);
    assert_eq!(
        lines.lines().next().map(|l| l.to),
        Some(Vec3::ONE),
        "the oldest went"
    );
}

/// Opening another scene in edit mode, where no tick ages them, drops the lines
/// drawn over the old one.
#[test]
fn applying_a_document_drops_the_lines() {
    let mut scene = crate::scene::Scene::new();
    scene.debug_lines.push(Vec3::ZERO, Vec3::X, WHITE, 7);
    let data = crate::scene::to_scene_data(&crate::scene::Scene::new());
    crate::scene::apply_scene_data(&mut scene, data);
    assert!(scene.debug_lines.is_empty());
}
