//! A send that moves: how much of a part reaches the song's room, across the piece.

use rusty_zimmer::patch::Fx;
use rusty_zimmer::song::{InlineOnly, Param, Song};
use rusty_zimmer::{render_song, SynthError};

use super::setup::{at, curve, frame, held, ramp, render, rms, BEATS};

fn roomy(send: f32, moving: Option<rusty_zimmer::song::Automation>) -> Song {
    let mut song = held(0.5);
    song.fx = vec![Fx::Reverb {
        size: 0.8,
        damp: 0.3,
        mix: 0.4,
        predelay: 0.0,
    }];
    song.tracks[0].send = send;
    song.automation = moving.into_iter().collect();
    song
}

fn tail(mix: &[f32]) -> &[f32] {
    &mix[frame(BEATS) + 2_000..]
}

#[test]
fn a_flat_send_curve_renders_what_the_written_send_did() {
    let written = render(&roomy(0.4, None));
    let parked = render(&roomy(1.0, Some(curve(Param::Send, vec![at(0.0, 0.4)]))));
    assert_eq!(written.len(), parked.len());
    let worst = written
        .iter()
        .zip(&parked)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 1e-6, "they differ by up to {worst}");
}

#[test]
fn a_send_opening_across_the_piece_washes_the_part_out() {
    let dry = render(&roomy(0.0, None));
    let wet = render(&roomy(1.0, None));
    let washing = render(&roomy(1.0, Some(ramp(Param::Send, 0.0, 1.0))));
    let window = |mix: &[f32], beat: f32| rms(&mix[frame(beat)..frame(beat) + 2_000]).to_owned();
    let start = window(&washing, 0.1);
    assert!(
        (start - window(&dry, 0.1)).abs() < 1e-3,
        "the first beat is dry: {start} against {}",
        window(&dry, 0.1)
    );
    assert!(
        (start - window(&wet, 0.1)).abs() > 1e-3,
        "and not wet: the fixture must tell the two apart"
    );
    let rang = rms(tail(&washing));
    assert_eq!(rms(tail(&dry)), 0.0, "a dry part leaves no tail");
    assert!(
        rang > 0.25 * rms(tail(&wet)),
        "the end was sent and rings: {rang} against {}",
        rms(tail(&wet))
    );
    assert!(
        rang <= rms(tail(&wet)) * 1.0001,
        "never more than all of it"
    );
}

#[test]
fn a_send_curve_past_its_range_is_clamped() {
    let over = render(&roomy(1.0, Some(curve(Param::Send, vec![at(0.0, 3.0)]))));
    let full = render(&roomy(1.0, Some(curve(Param::Send, vec![at(0.0, 1.0)]))));
    assert_eq!(over, full);
    let under = render(&roomy(1.0, Some(curve(Param::Send, vec![at(0.0, -2.0)]))));
    assert_eq!(rms(tail(&under)), 0.0, "below nothing is nothing");
}

#[test]
fn a_send_curve_with_no_room_to_reach_is_refused() {
    let mut song = held(0.5);
    song.automation = vec![ramp(Param::Send, 0.0, 1.0)];
    song.fx = vec![Fx::Saturate {
        drive: 1.2,
        mix: 0.5,
    }];
    assert!(matches!(
        song.validate().expect_err("nothing hears a send"),
        SynthError::AutomationWithoutRoom { ref track } if track == "pad"
    ));
    let refusal = render_song(&song, &InlineOnly)
        .expect_err("refused at render too")
        .to_string();
    assert!(
        refusal.contains("reverb") && refusal.contains("delay"),
        "it names what would make it audible: {refusal}"
    );
    song.fx = vec![Fx::Delay {
        time: 0.25,
        beats: None,
        feedback: 0.3,
        mix: 0.3,
        ping_pong: false,
    }];
    song.validate().expect("a delay is a room for a send");
}

#[test]
fn send_is_spelled_the_way_the_document_writes_it() {
    assert_eq!(Param::Send.as_str(), "send");
    let read: rusty_zimmer::song::Automation =
        serde_json::from_str(r#"{ "track": "pad", "param": "send", "points": [] }"#)
            .expect("`send` is a parameter");
    assert_eq!(read.param, Param::Send);
}
