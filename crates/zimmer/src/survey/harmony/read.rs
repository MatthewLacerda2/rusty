//! The arithmetic behind a [`Harmony`](super::Harmony): which notes are read, which of them
//! overlap a semitone apart, and which leave the key.

use std::collections::HashMap;

use super::{CLASHES_LISTED, Clash, PASSING, Place, STRAYS_LISTED, Stray, Voice};
use crate::song::shape::plan;
use crate::song::{PatchResolver, Song};
use crate::survey::song::{instrument, source};
use crate::survey::walk::{Sounded, walk};

/// A note the report reads: on a pitched track, with a pitch.
#[derive(Debug, Clone, Copy)]
pub(super) struct Held {
    /// Its track, by index into the song's tracks.
    track: usize,
    /// The pitch it sounds at, in MIDI.
    midi: f32,
    /// When it starts and stops, in beats from the top of one pass.
    start: f32,
    end: f32,
    /// The arrangement entry it starts in, and where that entry's slot starts.
    entry: usize,
    slot: f32,
    /// A copy of a note ringing past the end of the arrangement, moved back by
    /// one pass, because the piece plays again under it.
    wrapped: bool,
}

/// Every note on a pitched track, with a pitch, sorted by onset — and, when
/// the arrangement plays more than once, a copy of each note still ringing at
/// the end, laid over the top of the next pass.
pub(super) fn pitched(song: &Song, resolve: &dyn PatchResolver) -> Vec<Held> {
    let mut stated = vec![false; song.tracks.len()];
    let mut notes = Vec::new();
    walk(song, |note: Sounded| {
        let Some(track) = song.tracks.iter().position(|it| it.name == note.track) else {
            return;
        };
        stated[track] |= note.stated;
        let Some(midi) = note.midi else {
            return;
        };
        notes.push(Held {
            track,
            midi,
            start: note.start,
            end: note.start + note.dur,
            entry: note.entry,
            slot: note.slot,
            wrapped: false,
        });
    });
    let noise: Vec<bool> = song
        .tracks
        .iter()
        .map(|track| {
            instrument(&track.patch, resolve).is_some_and(|patch| source(&patch) == "noise")
        })
        .collect();
    notes.retain(|note| stated[note.track] && !noise[note.track]);
    let (_, passes) = plan(song);
    let total = song.arrangement_beats();
    if passes > 1 && total > 0.0 {
        let over: Vec<Held> = notes
            .iter()
            .filter(|note| note.end > total)
            .map(|note| Held {
                start: note.start - total,
                end: note.end - total,
                wrapped: true,
                ..*note
            })
            .collect();
        notes.extend(over);
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    notes
}

/// Whether two pitches are a semitone apart in any octave.
fn rub(a: f32, b: f32) -> bool {
    let apart = (a.round() as i32 - b.round() as i32).rem_euclid(12);
    apart == 1 || apart == 11
}

/// What a finding is grouped by: the same notes, on the same tracks, at the
/// same beat of a section of the same name, are one finding however many
/// times the arrangement plays it.
type Group = (String, i64, usize, i64, usize, i64);

/// A beat or a pitch as a key: thousandths, rounded.
fn whole(n: f32) -> i64 {
    (f64::from(n) * 1000.0).round() as i64
}

/// The clashes in `notes`: how many, how long in all, and the longest few.
pub(super) fn clashes(song: &Song, notes: &[Held]) -> (usize, f32, Vec<Clash>) {
    let mut found: Vec<(Group, Clash)> = Vec::new();
    let mut at: HashMap<Group, usize> = HashMap::new();
    let (mut count, mut total) = (0, 0.0);
    for (i, first) in notes.iter().enumerate() {
        for second in notes[i + 1..].iter().take_while(|it| it.start < first.end) {
            if first.track == second.track
                || (first.wrapped && second.wrapped)
                || !rub(first.midi, second.midi)
            {
                continue;
            }
            // Sorted by onset, so the overlap opens where the second begins.
            let beats = first.end.min(second.end) - second.start;
            if beats < PASSING {
                continue;
            }
            count += 1;
            total += beats;
            let (low, high) = if first.track < second.track {
                (first, second)
            } else {
                (second, first)
            };
            let place = place(song, second);
            let group = (
                place.section.clone(),
                whole(place.beat),
                low.track,
                whole(low.midi),
                high.track,
                whole(high.midi),
            );
            match at.get(&group) {
                Some(&index) => {
                    let clash = &mut found[index].1;
                    clash.at.times += 1;
                    clash.beats = clash.beats.max(beats);
                }
                None => {
                    at.insert(group.clone(), found.len());
                    let voice = |note: &Held| Voice {
                        track: song.tracks[note.track].name.clone(),
                        midi: note.midi,
                    };
                    found.push((
                        group,
                        Clash {
                            low: voice(low),
                            high: voice(high),
                            beats,
                            at: place,
                        },
                    ));
                }
            }
        }
    }
    let mut longest: Vec<Clash> = found.into_iter().map(|(_, clash)| clash).collect();
    // Stable, so a tie keeps the order the piece reaches them in.
    longest.sort_by(|a, b| {
        b.beats
            .total_cmp(&a.beats)
            .then(b.at.times.cmp(&a.at.times))
    });
    longest.truncate(CLASHES_LISTED);
    (count, total, longest)
}

/// One track's notes that `inside` says are outside the key: their share of
/// its held time and the longest few, or nothing when there are none.
///
/// A wrapped copy is the same note again, so it is not counted twice.
pub(super) fn strays(
    song: &Song,
    notes: &[Held],
    track: usize,
    inside: impl Fn(f32) -> bool,
) -> Option<(f32, Vec<Stray>)> {
    let mut held = 0.0;
    let mut outside = 0.0;
    let mut found: Vec<Stray> = Vec::new();
    for note in notes.iter().filter(|it| it.track == track && !it.wrapped) {
        let beats = note.end - note.start;
        held += beats;
        if inside(note.midi) || beats < PASSING {
            continue;
        }
        outside += beats;
        let at = place(song, note);
        match found.iter_mut().find(|stray| {
            stray.at.section == at.section
                && whole(stray.at.beat) == whole(at.beat)
                && whole(stray.midi) == whole(note.midi)
        }) {
            Some(stray) => {
                stray.at.times += 1;
                stray.beats = stray.beats.max(beats);
            }
            None => found.push(Stray {
                midi: note.midi,
                beats,
                at,
            }),
        }
    }
    if found.is_empty() || held <= 0.0 {
        return None;
    }
    found.sort_by(|a, b| {
        b.beats
            .total_cmp(&a.beats)
            .then(b.at.times.cmp(&a.at.times))
    });
    found.truncate(STRAYS_LISTED);
    Some((outside / held, found))
}

/// Where `note` starts, as a section and a beat into it.
///
/// Never asked of a wrapped copy: a clash is placed at the later onset, which
/// a copy moved back a pass never is, and strays skip copies.
fn place(song: &Song, note: &Held) -> Place {
    Place {
        section: song
            .arrangement
            .get(note.entry)
            .map(|entry| entry.label())
            .unwrap_or_default(),
        beat: note.start - note.slot,
        times: 1,
    }
}
