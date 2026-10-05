//! The speech brief (#386): its four local refusals and its canonical form. Nothing
//! here reaches a key, a budget or a network.

mod generate;

use scorsese_core::{SpeechModel, MAX_CHARACTERS};

use super::super::generated::{canonical, Brief};
use super::super::speech::*;

pub(super) fn draft(text: &str) -> Draft {
    Draft {
        text: Some(text.into()),
        voice: Some("voice-123".into()),
        ..Draft::default()
    }
}

#[test]
fn flash_is_the_default_model() {
    let brief = draft("Contact, second floor.").gather().unwrap();
    assert_eq!(brief.model, SpeechModel::Fast);
    assert_eq!(DEFAULT_MODEL.model_id(), "eleven_flash_v2_5");
    assert_eq!((brief.kind(), brief.extension()), ("speech", "wav"));
}

#[test]
fn naming_the_default_model_and_omitting_it_hash_alike() {
    let named = Draft {
        model: Some("eleven_flash_v2_5".into()),
        ..draft("Reloading!")
    };
    let a = canonical(&named.gather().unwrap()).unwrap();
    let b = canonical(&draft("Reloading!").gather().unwrap()).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a["model"], "eleven_flash_v2_5",
        "the vendor's id is what is hashed"
    );
    assert!(a.get("seed").is_none() && a.get("language").is_none());
}

#[test]
fn every_vendor_model_id_is_accepted() {
    for id in ["eleven_flash_v2_5", "eleven_multilingual_v2", "eleven_v3"] {
        let brief = Draft {
            model: Some(id.into()),
            ..draft("Flank left!")
        };
        assert_eq!(brief.gather().unwrap().model.model_id(), id);
    }
}

#[test]
fn empty_text_is_refused() {
    for text in [None, Some(""), Some("   ")] {
        let d = Draft {
            text: text.map(Into::into),
            ..draft("")
        };
        assert_eq!(d.gather().unwrap_err(), Incomplete::NoText);
    }
    assert!(Incomplete::NoText.to_string().contains("`text`"));
}

#[test]
fn text_past_the_vendor_limit_is_refused_by_character_not_byte() {
    // Multi-byte characters: the limit counts what the vendor counts.
    let at_limit = "é".repeat(MAX_CHARACTERS);
    assert!(draft(&at_limit).gather().is_ok());
    let over = format!("{at_limit}é");
    let why = draft(&over).gather().unwrap_err();
    assert_eq!(
        why,
        Incomplete::TooLong {
            found: MAX_CHARACTERS + 1
        }
    );
    assert!(why.to_string().contains("40000"));
}

#[test]
fn a_missing_voice_is_refused() {
    for voice in [None, Some("")] {
        let d = Draft {
            voice: voice.map(Into::into),
            ..draft("He's behind the crates")
        };
        assert_eq!(d.gather().unwrap_err(), Incomplete::NoVoice);
    }
    assert!(Incomplete::NoVoice.to_string().contains("`voice`"));
}

#[test]
fn language_on_multilingual_v2_is_refused() {
    let d = Draft {
        model: Some("eleven_multilingual_v2".into()),
        language: Some("pt".into()),
        ..draft("Granada!")
    };
    let why = d.gather().unwrap_err();
    assert_eq!(
        why,
        Incomplete::LanguageIgnored {
            language: "pt".into()
        }
    );
    assert!(why.to_string().contains("eleven_multilingual_v2"));
    // The models that honour it take it.
    for id in ["eleven_flash_v2_5", "eleven_v3"] {
        let d = Draft {
            model: Some(id.into()),
            language: Some("pt".into()),
            ..draft("Granada!")
        };
        assert_eq!(d.gather().unwrap().language.as_deref(), Some("pt"));
    }
}

#[test]
fn an_unknown_model_is_refused_naming_the_three() {
    let d = Draft {
        model: Some("eleven_turbo_v2_5".into()),
        ..draft("Clear!")
    };
    let why = d.gather().unwrap_err().to_string();
    assert!(
        why.contains("eleven_turbo_v2_5") && why.contains("eleven_v3"),
        "{why}"
    );
}

#[test]
fn the_estimate_matches_the_rate_table() {
    // Flash 5¢ and v3 10¢ per thousand characters, rounded up.
    let estimate = |d: Draft| estimate_cents(&d.gather().unwrap()).unwrap();
    assert_eq!(estimate(draft("Contact, second floor.")), 1);
    assert_eq!(estimate(draft(&"a".repeat(3000))), 15);
    let v3 = Draft {
        model: Some("eleven_v3".into()),
        ..draft(&"a".repeat(3001))
    };
    assert_eq!(estimate(v3), 31);
}
