//! Walking a parsed file into what the importer needs: the notes each part
//! plays, paired, and the conductor's events — still in ticks.
//!
//! Nothing here decides anything musical. A note is a note-on paired with the
//! note-off that ends it; a part is one MIDI track playing on one channel; a
//! tempo, a meter and a key are what the file wrote and where. Everything a
//! song *is* gets built from this in `build`, which is the half that has
//! opinions.

use std::collections::{BTreeMap, VecDeque};

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

use super::MidiError;

/// The channel General MIDI reserves for percussion — channel 10 as a musician
/// counts, 9 as the wire does.
pub(super) const DRUM_CHANNEL: u8 = 9;

/// Everything the importer reads out of a file, in ticks.
#[derive(Debug, Default)]
pub(super) struct Score {
    /// Ticks per quarter note — the file's own resolution.
    pub(super) ppq: u16,
    /// The parts, in the order the file first plays them.
    pub(super) parts: Vec<Part>,
    /// `(tick, microseconds per quarter)`, as written, in file order.
    pub(super) tempos: Vec<(u64, u32)>,
    /// `(tick, numerator, denominator as a power of two)`.
    pub(super) meters: Vec<(u64, u8, u8)>,
    /// `(tick, sharps — negative for flats, minor)`.
    pub(super) keys: Vec<(u64, i8, bool)>,
    /// What the file carries that a song has nowhere to put.
    pub(super) unread: Unread,
}

/// One MIDI track playing on one channel: what becomes one song track.
#[derive(Debug)]
pub(super) struct Part {
    /// Which MIDI track it came from, counting from zero.
    pub(super) track: usize,
    /// Which channel, as the wire counts (`0..16`).
    pub(super) channel: u8,
    /// The track's own name, if it wrote one.
    pub(super) name: Option<String>,
    /// The first program the channel was set to on this track, if any.
    pub(super) program: Option<u8>,
    /// Its notes, in the order they end up paired.
    pub(super) notes: Vec<Held>,
}

impl Part {
    /// Whether this part is on the percussion channel.
    pub(super) fn is_drums(&self) -> bool {
        self.channel == DRUM_CHANNEL
    }
}

/// One sounded note, in ticks.
#[derive(Debug, Clone, Copy)]
pub(super) struct Held {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) key: u8,
    pub(super) vel: u8,
}

/// Counts of what was in the file and is not in the song.
///
/// Counted rather than dropped quietly, because an import that says nothing
/// about the sustain pedal leaves a pianist wondering why the piece sounds
/// dry, and the answer is a sentence the importer already knows.
#[derive(Debug, Default)]
pub(super) struct Unread {
    pub(super) controllers: usize,
    pub(super) bends: usize,
    pub(super) aftertouch: usize,
    pub(super) sysex: usize,
    /// Notes the file never released, ended where their track ends.
    pub(super) unreleased: usize,
    /// Notes whose release fell on their own onset, given one tick.
    pub(super) instant: usize,
}

/// Reads `smf` into a [`Score`], or refuses a file whose clock is not beats.
pub(super) fn score(smf: &Smf<'_>) -> Result<Score, MidiError> {
    let ppq = match smf.header.timing {
        Timing::Metrical(ppq) if ppq.as_int() > 0 => ppq.as_int(),
        Timing::Metrical(_) => return Err(MidiError::NoResolution),
        Timing::Timecode(..) => return Err(MidiError::Timecode),
    };
    if smf.header.format == Format::Sequential {
        return Err(MidiError::Sequential);
    }
    let mut score = Score {
        ppq,
        ..Score::default()
    };
    for (index, events) in smf.tracks.iter().enumerate() {
        walk(index, events, &mut score);
    }
    Ok(score)
}

/// One MIDI track's events, into the score.
fn walk(index: usize, events: &[midly::TrackEvent<'_>], score: &mut Score) {
    let mut tick = 0_u64;
    let mut name = None;
    let mut parts: BTreeMap<u8, Part> = BTreeMap::new();
    let mut order: Vec<u8> = Vec::new();
    // Per channel and key, the onsets still sounding — a queue, so a key
    // struck twice before either is released pairs first-on with first-off.
    let mut open: BTreeMap<(u8, u8), VecDeque<(u64, u8)>> = BTreeMap::new();

    for event in events {
        tick += u64::from(event.delta.as_int());
        match event.kind {
            TrackEventKind::Midi { channel, message } => {
                let channel = channel.as_int();
                let part = parts.entry(channel).or_insert_with(|| {
                    order.push(channel);
                    Part {
                        track: index,
                        channel,
                        name: None,
                        program: None,
                        notes: Vec::new(),
                    }
                });
                play(tick, channel, message, part, &mut open, &mut score.unread);
            }
            TrackEventKind::Meta(meta) => conduct(tick, meta, &mut name, score),
            TrackEventKind::SysEx(_) | TrackEventKind::Escape(_) => score.unread.sysex += 1,
        }
    }

    // What was still held when the track ran out ends where the track does:
    // the file said the note started and never said it stopped, and the end
    // of its track is the last moment the file speaks for.
    for ((channel, key), onsets) in open {
        for (start, vel) in onsets {
            score.unread.unreleased += 1;
            if let Some(part) = parts.get_mut(&channel) {
                part.notes
                    .push(held(start, tick, key, vel, &mut score.unread));
            }
        }
    }
    for channel in order {
        let Some(mut part) = parts.remove(&channel) else {
            continue;
        };
        if part.notes.is_empty() {
            continue;
        }
        part.name.clone_from(&name);
        part.notes.sort_by_key(|note| (note.start, note.key));
        score.parts.push(part);
    }
}

/// One channel message.
fn play(
    tick: u64,
    channel: u8,
    message: MidiMessage,
    part: &mut Part,
    open: &mut BTreeMap<(u8, u8), VecDeque<(u64, u8)>>,
    unread: &mut Unread,
) {
    match message {
        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
            open.entry((channel, key.as_int()))
                .or_default()
                .push_back((tick, vel.as_int()));
        }
        // A note-on at velocity zero is a release: the running-status idiom
        // nearly every sequencer writes, and the spec's own.
        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
            let key = key.as_int();
            // A release with nothing to release carries no sound, so there is
            // nothing to report about it.
            if let Some((start, vel)) = open.get_mut(&(channel, key)).and_then(VecDeque::pop_front)
            {
                part.notes.push(held(start, tick, key, vel, unread));
            }
        }
        MidiMessage::ProgramChange { program } => {
            part.program.get_or_insert(program.as_int());
        }
        MidiMessage::Controller { .. } => unread.controllers += 1,
        MidiMessage::PitchBend { .. } => unread.bends += 1,
        MidiMessage::Aftertouch { .. } | MidiMessage::ChannelAftertouch { .. } => {
            unread.aftertouch += 1;
        }
    }
}

/// A paired note, given one tick if the file released it where it started —
/// the shortest length the file's own clock can say, and the reading that
/// keeps a drum hit written as on-and-off-at-once from vanishing.
fn held(start: u64, end: u64, key: u8, vel: u8, unread: &mut Unread) -> Held {
    let end = if end > start {
        end
    } else {
        unread.instant += 1;
        start + 1
    };
    Held {
        start,
        end,
        key,
        vel,
    }
}

/// One meta event: the conductor's, or the track's name.
fn conduct(tick: u64, meta: MetaMessage<'_>, name: &mut Option<String>, score: &mut Score) {
    match meta {
        MetaMessage::TrackName(bytes) => {
            let text = String::from_utf8_lossy(bytes).trim().to_owned();
            if name.is_none() && !text.is_empty() {
                *name = Some(text);
            }
        }
        MetaMessage::Tempo(tempo) if tempo.as_int() > 0 => {
            score.tempos.push((tick, tempo.as_int()))
        }
        // A denominator past 1/64 is not a meter anyone writes; it is a corrupt
        // byte, and read literally it would make every bar a fraction of a tick.
        MetaMessage::TimeSignature(numerator, power, ..) if numerator > 0 && power <= 6 => {
            score.meters.push((tick, numerator, power));
        }
        MetaMessage::KeySignature(sharps, minor) => score.keys.push((tick, sharps, minor)),
        _ => {}
    }
}
