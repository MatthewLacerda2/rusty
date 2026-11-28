use super::plan::{plan, Candidate};

/// A square RGBA8 chain of `side` texels.
fn texture(side: u32, dropped: u32, pixels: Option<f32>) -> Candidate {
    let full = 32 - side.leading_zeros();
    let levels = (0..full).map(|m| u64::from((side >> m).max(1)).pow(2) * 4);
    Candidate {
        side,
        dropped,
        most: full - 1,
        pixels,
        levels: levels.collect(),
    }
}

#[test]
fn a_seen_texture_keeps_what_its_screen_size_needs_and_one_level_more() {
    let near = texture(1024, 3, Some(1024.0));
    let far = texture(1024, 0, Some(16.0));
    let got = plan(&[near, far], 0, None).targets;
    assert_eq!(
        got,
        [0, 5],
        "1024 texels on 16 pixels needs 64, kept at 32's level up"
    );
}

#[test]
fn a_texture_moves_down_only_when_two_levels_over_its_need() {
    let one_over = texture(1024, 0, Some(200.0));
    let two_over = texture(1024, 0, Some(100.0));
    assert_eq!(plan(&[one_over, two_over], 0, None).targets, [0, 2]);
}

#[test]
fn an_unseen_texture_keeps_its_levels_under_the_budget() {
    let unseen = texture(256, 1, None);
    assert_eq!(plan(&[unseen], 0, Some(u64::MAX)).targets, [1]);
}

#[test]
fn over_the_budget_the_unseen_then_the_farthest_lose_levels_first() {
    let near = texture(256, 0, Some(256.0));
    let far = texture(256, 0, Some(64.0));
    let unseen = texture(256, 0, None);
    let need: u64 = [&near, &far, &unseen].iter().map(|c| c.bytes(0)).sum();
    let budget = need - 300_000;
    let got = plan(&[near, far, unseen], 0, Some(budget));
    assert!(!got.over);
    assert_eq!(got.targets[0], 0, "the nearest keeps its top level");
    assert!(
        got.targets[2] >= 1,
        "the unseen goes first: {:?}",
        got.targets
    );
}

#[test]
fn the_budget_holds_the_total_and_says_when_it_cannot() {
    let maps: Vec<_> = (0..4).map(|_| texture(512, 0, Some(512.0))).collect();
    let budget = 1 << 20;
    let got = plan(&maps, 0, Some(budget));
    let total: u64 = maps
        .iter()
        .zip(&got.targets)
        .map(|(c, &t)| c.bytes(t))
        .sum();
    assert!(total <= budget && !got.over, "{total} over {budget}");
    let crowded = plan(&maps, 2 << 20, Some(budget));
    assert!(crowded.over, "what does not stream already passes it");
    assert!(crowded.targets.iter().zip(&maps).all(|(&t, c)| t == c.most));
}

#[test]
fn an_eye_inside_the_bounds_needs_every_level_and_a_nan_size_keeps_its_own() {
    let inside = texture(512, 4, Some(f32::INFINITY));
    let nan = texture(512, 4, Some(f32::NAN));
    assert_eq!(plan(&[inside, nan], 0, None).targets, [0, 4]);
}
