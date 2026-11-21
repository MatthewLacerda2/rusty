//! Played parts and a tempo map, encoded: the conductor track, one track per
//! part, and the bytes.
//!
//! The encoder is `midly`'s, and nothing of it leaves this file. What is
//! decided here is only the order of events on one tick: **every release
//! before any strike**, so a note struck again on the tick the last one ends
//! is a new note rather than a release of itself — which is how the importer
//! pairs them back up, first on with first off.

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use super::play::Part;
use super::{ExportError, TICKS_PER_BEAT};
use crate::Song;
use crate::midi::sound;

/// The file's bytes, and a sentence if the song's key had no signature to be
/// written as.
pub(super) fn file(
    song: &Song,
    parts: &[Part<'_>],
    tempos: &[(u64, u32)],
) -> Result<(Vec<u8>, Vec<String>), ExportError> {
    let (conductor, left_out) = conductor(song, tempos)?;
    let mut tracks = vec![conductor];
    for part in parts {
        tracks.push(track(part)?);
    }
    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(TICKS_PER_BEAT))),
        tracks,
    };
    let mut bytes = Vec::new();
    smf.write(&mut bytes)
        .map_err(|why| ExportError::Unwritable {
            why: why.to_owned(),
        })?;
    Ok((bytes, left_out))
}

/// The conductor track: the key signature, if the song's key has one, and
/// every tempo.
fn conductor<'s>(
    song: &'s Song,
    tempos: &[(u64, u32)],
) -> Result<(Vec<TrackEvent<'s>>, Vec<String>), ExportError> {
    let mut timed = Vec::new();
    let mut left_out = Vec::new();
    // Validated before this is reached, so a key that does not read is not
    // one this can meet; it is skipped rather than assumed.
    if let (Some(written), Ok(Some(key))) = (song.key.as_deref(), song.key()) {
        match sound::signature(written, &key) {
            Some((sharps, minor)) => {
                timed.push((
                    0,
                    TrackEventKind::Meta(MetaMessage::KeySignature(sharps, minor)),
                ));
            }
            None => left_out.push(format!(
                "the key `{written}` has no MIDI key signature — a signature is major or \
                 minor — so the file declares none; every note is still its own pitch"
            )),
        }
    }
    for &(tick, micros) in tempos {
        timed.push((
            tick,
            TrackEventKind::Meta(MetaMessage::Tempo(u24::new(micros))),
        ));
    }
    Ok((delta_timed(timed)?, left_out))
}

/// One part as a MIDI track: its name, then its notes.
fn track<'s>(part: &Part<'s>) -> Result<Vec<TrackEvent<'s>>, ExportError> {
    let channel = u4::new(part.channel);
    // `(tick, 0 for a release and 1 for a strike, key)`, sorted, is the order
    // the module doc gives.
    let mut edges: Vec<(u64, u8, u8, u8)> = Vec::with_capacity(part.notes.len() * 2);
    for note in &part.notes {
        edges.push((note.start, 1, note.key, note.vel));
        edges.push((note.end, 0, note.key, 0));
    }
    edges.sort_by_key(|&(tick, strike, key, _)| (tick, strike, key));
    let mut timed = vec![(
        0,
        TrackEventKind::Meta(MetaMessage::TrackName(part.name.as_bytes())),
    )];
    for (tick, strike, key, vel) in edges {
        let key = u7::new(key);
        let message = if strike == 1 {
            MidiMessage::NoteOn {
                key,
                vel: u7::new(vel),
            }
        } else {
            MidiMessage::NoteOff {
                key,
                vel: u7::new(0),
            }
        };
        timed.push((tick, TrackEventKind::Midi { channel, message }));
    }
    delta_timed(timed)
}

/// Events at absolute ticks, already in order, as the delta-timed events a
/// track holds — closed with the end-of-track event on the last tick.
fn delta_timed(timed: Vec<(u64, TrackEventKind<'_>)>) -> Result<Vec<TrackEvent<'_>>, ExportError> {
    let mut now = 0;
    let mut events = Vec::with_capacity(timed.len() + 1);
    for (tick, kind) in timed {
        let delta = u32::try_from(tick - now)
            .ok()
            .and_then(u28::try_from)
            .ok_or(ExportError::TooLong)?;
        now = tick;
        events.push(TrackEvent { delta, kind });
    }
    events.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(events)
}
