//! Clicks and dropouts, where they are, and how many.

use scorsese_zimmer::level::DefectFinder;

use super::{RATE, found, interleaved, steady, tone};

#[test]
fn a_jump_in_a_smooth_tone_is_a_click_at_its_moment() {
    let mut signal = tone(220.0, 0.4, 2.0, steady);
    // Everything from 1.25 s on is shifted up by a fifth of full scale: one
    // step discontinuity, nowhere else.
    let at = (1.25 * f64::from(RATE)) as usize;
    for sample in &mut signal[at..] {
        *sample += 0.2;
    }
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 1, "{defects:?}");
    assert!(defects.dropouts.count == 0 && defects.zipper.count == 0);
    let click = &defects.clicks.worst[0];
    assert!((click.at_seconds - 1.25).abs() < 0.001, "{click:?}");
    assert!(click.size_db > 12.0, "{click:?}");
    assert_eq!(
        click.channel, None,
        "a mono file's defect is in every channel"
    );
}

#[test]
fn a_gap_in_continuous_sound_is_a_dropout_with_its_length() {
    let mut signal = tone(440.0, 0.4, 2.0, steady);
    let from = RATE as usize;
    let to = from + (0.020 * f64::from(RATE)) as usize;
    signal[from..to].fill(0.0);
    let defects = found(1, &signal);
    assert_eq!(defects.dropouts.count, 1, "{defects:?}");
    assert_eq!(defects.zipper.count, 0, "{defects:?}");
    let gap = &defects.dropouts.worst[0];
    assert!((gap.at_seconds - 1.0).abs() < 0.001, "{gap:?}");
    let length = gap.to_seconds - gap.from_seconds;
    assert!((length - 0.020).abs() < 0.0015, "{gap:?}");
    assert!(gap.size_db >= 40.0, "{gap:?}");
}

/// A pause is not a dropout: speech stops between sentences, and a song
/// stops before its last chord.
#[test]
fn a_pause_longer_than_a_dropout_is_not_one() {
    let mut signal = tone(440.0, 0.4, 3.0, steady);
    signal[RATE as usize..2 * RATE as usize].fill(0.0);
    assert_eq!(found(1, &signal).dropouts.count, 0);
}

#[test]
fn a_defect_in_one_channel_says_which() {
    let clean = tone(440.0, 0.4, 2.0, steady);
    let mut gapped = clean.clone();
    gapped[RATE as usize..RATE as usize + 480].fill(0.0);

    let left = found(2, &interleaved(&gapped, &clean));
    assert_eq!(left.dropouts.count, 1, "{left:?}");
    assert_eq!(left.dropouts.worst[0].channel, Some(0));

    let both = found(2, &interleaved(&gapped, &gapped));
    assert_eq!(both.dropouts.count, 1, "{both:?}");
    assert_eq!(both.dropouts.worst[0].channel, None);
}

/// A render feeds its mix a segment at a time, so the answer cannot depend on
/// where the runs were cut.
#[test]
fn a_signal_fed_in_runs_reads_as_it_does_fed_whole() {
    let mut signal = tone(220.0, 0.4, 2.0, steady);
    for sample in &mut signal[RATE as usize..] {
        *sample += 0.2;
    }
    signal[(1.5 * f64::from(RATE)) as usize..(1.51 * f64::from(RATE)) as usize].fill(0.0);
    let whole = found(1, &signal);
    let mut finder = DefectFinder::new(1, RATE);
    for run in signal.chunks(1_001) {
        finder.feed(run);
    }
    assert_eq!(finder.finish(), whole);
}

#[test]
fn the_worst_are_kept_and_the_rest_counted() {
    let mut signal = tone(220.0, 0.4, 4.0, steady);
    for (n, at) in (0..12).map(|n| (n, 0.2 + 0.3 * f64::from(n))) {
        let at = (at * f64::from(RATE)) as usize;
        signal[at] += 0.1 + 0.02 * n as f32;
    }
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 12, "{defects:?}");
    assert_eq!(defects.clicks.worst.len(), 5);
    assert!(
        defects
            .clicks
            .worst
            .windows(2)
            .all(|pair| pair[0].size_db >= pair[1].size_db),
        "worst first"
    );
}
