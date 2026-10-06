//! The sound-effect brief (#388): its local refusals, its canonical form and its
//! estimate. Nothing here reaches a key, a budget or a network.

mod generate;

use super::super::generated::{canonical, Brief};
use super::super::sfx::*;

pub(super) fn draft(text: &str, seconds: f64) -> Draft {
    Draft {
        text: Some(text.into()),
        seconds: Some(seconds),
        ..Draft::default()
    }
}

#[test]
fn a_brief_is_a_wav_named_sfx_pinned_to_the_one_model() {
    let brief = draft("door slam", 2.0).gather().unwrap();
    assert_eq!((brief.kind(), brief.extension()), ("sfx", "wav"));
    assert_eq!(brief.model, "eleven_text_to_sound_v2");
    assert_eq!(request(&brief).model_id, MODEL);
}

#[test]
fn naming_the_defaults_and_omitting_them_hash_alike() {
    let named = Draft {
        prompt_influence: Some(0.3),
        looping: Some(false),
        ..draft("glass shatter", 1.5)
    };
    let a = canonical(&named.gather().unwrap()).unwrap();
    let b = canonical(&draft("glass shatter", 1.5).gather().unwrap()).unwrap();
    assert_eq!(a, b);
    assert!(a.get("loop").is_none(), "no loop flag unless it loops");
    let looped = Draft {
        looping: Some(true),
        ..draft("glass shatter", 1.5)
    };
    assert_eq!(canonical(&looped.gather().unwrap()).unwrap()["loop"], true);
}

#[test]
fn the_length_is_part_of_the_address() {
    let a = canonical(&draft("reload", 1.0).gather().unwrap()).unwrap();
    let b = canonical(&draft("reload", 1.2).gather().unwrap()).unwrap();
    assert_ne!(a, b);
}

#[test]
fn empty_text_is_refused() {
    for text in [None, Some(""), Some("  ")] {
        let d = Draft {
            text: text.map(Into::into),
            ..draft("", 1.0)
        };
        assert_eq!(d.gather().unwrap_err(), Incomplete::NoText);
    }
}

#[test]
fn a_missing_length_is_refused_naming_the_bounds() {
    let d = Draft {
        seconds: None,
        ..draft("footstep on gravel", 1.0)
    };
    let why = d.gather().unwrap_err();
    assert_eq!(why, Incomplete::NoSeconds);
    assert!(why.to_string().contains("0.5") && why.to_string().contains("30"));
}

#[test]
fn a_length_the_endpoint_does_not_make_is_refused_locally() {
    for s in [0.49, 30.01, -1.0, f64::NAN, f64::INFINITY] {
        let why = draft("casing", s).gather().unwrap_err();
        assert!(matches!(why, Incomplete::SecondsOutOfRange(_)), "{s}");
        assert!(why.to_string().contains("`seconds`"));
    }
    for s in [MIN_SECONDS, MAX_SECONDS] {
        assert!(draft("casing", s).gather().is_ok());
    }
}

#[test]
fn a_prompt_influence_outside_zero_to_one_is_refused() {
    for p in [-0.1, 1.1] {
        let d = Draft {
            prompt_influence: Some(p),
            ..draft("cloth rustle", 1.0)
        };
        assert_eq!(d.gather().unwrap_err(), Incomplete::InfluenceOutOfRange(p));
    }
}

#[test]
fn the_estimate_is_the_dated_rate_per_second_rounded_up() {
    // 0.4¢ a second (price::MILLS_PER_SECOND), rounded up per call.
    assert_eq!(price::CHECKED, "2026-10-06");
    assert_eq!(estimate_cents(0.5), 1);
    assert_eq!(estimate_cents(2.5), 1);
    assert_eq!(estimate_cents(2.501), 2);
    assert_eq!(estimate_cents(10.0), 4);
    assert_eq!(estimate_cents(30.0), 12);
}
