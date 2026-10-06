//! The voice verbs' wiring on the dev surface (#387): they sit on `Speech`, refuse
//! during Play before any key is read, and reject an empty query without listing.

use super::super::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

#[test]
fn the_voice_verbs_sit_on_the_speech_table() {
    let surface = session().world().script_manager().api_surface();
    for verb in ["Generate", "Voices", "FindVoice", "VoiceInfo"] {
        assert!(
            surface["Speech"].contains(verb),
            "`Speech.{verb}` is missing"
        );
    }
}

#[test]
fn listing_voices_during_play_raises_before_any_key_is_read() {
    let s = session();
    *s.world().script_manager().play_state_cell().borrow_mut() = true;
    for call in [
        "Speech.Voices()",
        "Speech.FindVoice('calm')",
        "Speech.VoiceInfo('v')",
    ] {
        let err = s.eval(&format!("return {call}")).unwrap_err();
        assert!(err.contains("Play"), "{call}: {err}");
    }
}

#[test]
fn an_empty_voice_query_raises_without_listing() {
    let err = session().eval("return Speech.FindVoice('')").unwrap_err();
    assert!(err.contains("name or a label"), "{err}");
}
