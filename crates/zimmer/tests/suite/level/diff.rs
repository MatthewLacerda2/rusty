//! Two measurements, compared.

use scorsese_zimmer::level::{Difference, Profiler};
use scorsese_zimmer::{SAMPLE_RATE, bake_song, song::InlineOnly};

use super::{SLACK, common::songs, square};

fn profiled(samples: &[f32]) -> scorsese_zimmer::level::Profile {
    let mut profiler = Profiler::new(1, SAMPLE_RATE);
    profiler.feed(samples);
    profiler.finish()
}

#[test]
fn a_file_compared_with_itself_differs_by_nothing() {
    let bake = bake_song(&songs::song(), &InlineOnly).expect("the fixture song renders");
    let difference = Difference::between(&bake.profile.whole, &bake.profile.whole);
    assert!(difference.is_nothing(), "{difference:?}");
    assert_eq!(difference.mean_db, Some(0.0));
    assert_eq!(difference.peak_db, Some(0.0));
    assert_eq!(difference.seconds, 0.0);
    let bands = difference.bands.expect("a piece of music has a balance");
    assert_eq!(bands.largest(), 0.0);
}

#[test]
fn a_quieter_file_reads_as_exactly_that_many_decibels_quieter() {
    let loud = profiled(&square(1.0, 4_000));
    let quiet = profiled(&square(0.5, 4_000));

    let down = Difference::between(&quiet.whole, &loud.whole);
    assert!(!down.is_nothing());
    assert!((down.mean_db.expect("audible") + 6.02).abs() < SLACK);
    assert!((down.peak_db.expect("audible") + 6.02).abs() < SLACK);
    assert!(down.crest_db.expect("audible").abs() < SLACK);

    let up = Difference::between(&loud.whole, &quiet.whole);
    assert!((up.mean_db.expect("audible") - 6.02).abs() < SLACK);
}

#[test]
fn a_silence_has_no_difference_rather_than_a_difference_of_zero() {
    let sound = profiled(&square(0.5, 2_000));
    let silence = profiled(&[0.0; 2_000]);

    let difference = Difference::between(&sound.whole, &silence.whole);
    assert_eq!(difference.mean_db, None);
    assert_eq!(difference.peak_db, None);
    assert_eq!(difference.bands, None);
    // The lengths are still comparable, because a silence has one.
    assert!(difference.seconds.abs() < 0.001);
}

#[test]
fn a_narrower_version_reads_as_narrower_than_the_one_it_replaced() {
    let tone = |hz: f64| {
        move |frame: usize| {
            (std::f64::consts::TAU * hz * frame as f64 / f64::from(SAMPLE_RATE)).sin() as f32
        }
    };
    let stereo = |left: &dyn Fn(usize) -> f32, right: &dyn Fn(usize) -> f32| {
        let mut profiler = Profiler::new(2, SAMPLE_RATE);
        profiler.feed(
            &(0..4_000)
                .flat_map(|f| [left(f), right(f)])
                .collect::<Vec<_>>(),
        );
        profiler.finish()
    };
    let wide = stereo(&tone(440.0), &tone(997.0));
    let narrow = stereo(&tone(440.0), &tone(440.0));

    let inwards = Difference::between(&narrow.whole, &wide.whole);
    assert!(!inwards.is_nothing());
    let by = inwards.correlation.expect("both sides are two channels");
    assert!(by > 0.9, "going to mono is the largest move there is: {by}");
    let outwards = Difference::between(&wide.whole, &narrow.whole);
    assert!((by + outwards.correlation.expect("still two channels")).abs() < 1e-9);
}

#[test]
fn a_mono_file_has_no_width_to_be_compared_against() {
    let mut stereo = Profiler::new(2, SAMPLE_RATE);
    stereo.feed(&square(0.5, 8_000));
    let mono = profiled(&square(0.5, 4_000));
    let stereo = stereo.finish();
    assert_eq!(
        stereo.whole.correlation,
        Some(-1.0),
        "as far apart as it goes"
    );
    assert_eq!(mono.whole.correlation, None);

    let difference = Difference::between(&stereo.whole, &mono.whole);
    assert_eq!(
        difference.correlation, None,
        "against no width there is none"
    );
    assert_eq!(
        difference.peak_db,
        Some(0.0),
        "and the fields both sides do have still compare"
    );
}

#[test]
fn a_move_of_exactly_the_threshold_is_still_a_move() {
    let unmoved = Difference {
        mean_db: Some(0.0),
        peak_db: Some(0.0),
        crest_db: Some(0.0),
        bands: None,
        correlation: Some(0.0),
        seconds: 0.0,
    };
    assert!(unmoved.is_nothing(), "nothing moved at all");
    assert!(
        !Difference {
            mean_db: Some(0.01),
            ..unmoved
        }
        .is_nothing()
    );
    assert!(
        !Difference {
            correlation: Some(0.005),
            ..unmoved
        }
        .is_nothing()
    );
}
