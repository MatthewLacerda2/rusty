//! Chains that belong to a track or to the whole mix, rather than to a patch.

use crate::common::songs::{note, played, song, verse};
use crate::common::{channel, peak};
use rusty_zimmer::patch::Fx;
use rusty_zimmer::song::{InlineOnly, PatchRef, Song, Tail, Track};
use rusty_zimmer::{render_song, SAMPLE_RATE};

fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

fn arrangement_end(song: &Song) -> usize {
    (song.arrangement_beats() * song.beat_seconds() * SAMPLE_RATE as f32).round() as usize
}

fn room() -> Fx {
    Fx::Reverb {
        size: 0.5,
        damp: 0.5,
        mix: 0.3,
        predelay: 0.0,
    }
}

fn in_a_room() -> Song {
    Song {
        fx: vec![room()],
        ..song()
    }
}

#[test]
fn a_song_chain_colours_the_mix_and_rings_past_the_last_note() {
    let dry = render(&song());
    let wet = render(&in_a_room());
    assert!(
        wet.len() > dry.len(),
        "a wet mix has to grow by its own tail: {} vs {}",
        wet.len(),
        dry.len()
    );
    assert_ne!(wet[..dry.len()], dry[..], "and the music itself is changed");
    assert!(
        peak(&wet[dry.len()..]) > 0.0,
        "the extra length must be the tail, not padding"
    );
}

/// Close, not equal: per patch the room runs on each note and is summed in another order.
#[test]
fn a_room_on_the_song_is_the_room_every_patch_would_have_had_to_spell_out() {
    let mut per_patch = song();
    let PatchRef::Inline(patch) = &mut per_patch.tracks[0].patch else {
        panic!("the fixture carries its instrument inline")
    };
    patch.fx = vec![room()];

    let shared = render(&in_a_room());
    let copied = render(&per_patch);
    let overlap = shared.len().min(copied.len());
    let apart = shared[..overlap]
        .iter()
        .zip(&copied[..overlap])
        .fold(0.0f32, |most, (a, b)| most.max((a - b).abs()));
    assert!(
        apart < 0.02 * peak(&shared),
        "the two rooms differ by {apart}, which is not the same room"
    );
}

#[test]
fn a_track_chain_shapes_its_own_instrument_and_leaves_the_rest_alone() {
    let mut duet = song();
    duet.tracks.push(Track {
        name: "lead".to_owned(),
        ..duet.tracks[0].clone()
    });
    verse(&mut duet).notes = played(vec![
        note("bass", "E2", 0.0, 0.4),
        note("lead", "B4", 1.0, 0.4),
    ]);
    duet.arrangement = vec!["verse".into()];

    let mut on_the_lead = duet.clone();
    on_the_lead.tracks[1].fx = vec![room()];
    let plain = render(&duet);
    let coloured = render(&on_the_lead);

    // `bass` plays alone on the first beat, and it was never bussed, so that region has to be the
    // very same samples.
    let beat = (duet.beat_seconds() * SAMPLE_RATE as f32) as usize;
    assert_eq!(plain[..beat], coloured[..beat], "`bass` must be untouched");
    assert_ne!(
        plain[beat..beat * 2],
        coloured[beat..beat * 2],
        "`lead` must not be"
    );
    assert!(coloured.len() > plain.len(), "and its tail extends the mix");
}

#[test]
fn an_empty_chain_is_not_written_down_and_changes_no_sample() {
    let plain = song();
    let json = plain.to_json().expect("serialise");
    let written: serde_json::Value = serde_json::from_str(&json).expect("it is JSON");
    assert_eq!(written.get("fx"), None, "no mix chain was written:\n{json}");
    assert_eq!(
        written["tracks"][0].get("fx"),
        None,
        "and no track chain either:\n{json}"
    );
    assert_eq!(Song::from_json(&json).expect("deserialise"), plain);

    let mut spelled_out = song();
    spelled_out.fx = vec![];
    spelled_out.tracks[0].fx = vec![];
    assert_eq!(render(&spelled_out), render(&plain));
}

#[test]
fn tail_exact_still_lands_on_the_final_beat_with_a_mix_chain() {
    let cut = Song {
        tail: Some(Tail::Exact),
        ..in_a_room()
    };
    assert_eq!(render(&cut).len(), arrangement_end(&cut));
}
