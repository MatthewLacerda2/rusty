//! The two instruments an import hands every track, and how its notes are
//! spelled.
//!
//! **Plain on purpose.** A MIDI file names an instrument by a program number,
//! and turning that number into a piano is General MIDI emulation, which is a
//! sound library rather than a converter. So every pitched part gets the same
//! unremarkable voice and every drum part the same short burst: enough to hear
//! that the notes are right, with nothing in them anybody would mistake for a
//! choice. Choosing the sounds is the author's first edit, and a default that
//! sounded finished would be one they might not make.

use crate::patch::{Adsr, NoiseColor, Osc, Patch, Source, Wave};
use crate::song::{Key, Mode};

/// A pitched part's voice: one triangle, held for as long as the key is.
///
/// A sustaining envelope rather than a decaying one because a MIDI note says
/// how long it lasts, and a voice that died away on its own would hide that.
pub(super) fn pitched() -> Patch {
    plain(
        Source::OscStack {
            oscs: vec![Osc {
                wave: Wave::Triangle,
                detune_cents: 0.0,
                gain: 1.0,
                octave: 0,
                voices: 1,
                spread: 12.0,
                width: 0.0,
                pulse_width: 0.5,
            }],
        },
        Adsr {
            a: 0.005,
            d: 0.15,
            s: 0.7,
            r: 0.08,
            curve: 0.0,
        },
    )
}

/// A drum part's voice: a short burst of pink noise, the same for every key.
///
/// It decays on its own and releases as slowly as it decays, so a hit sounds
/// the same whether the file held the key for a tick or a beat — a drum
/// machine's key-down length is rarely anything the part meant. Every key
/// sounding alike is deliberate: which key is the kick is a General MIDI
/// convention, and the numbers stay in the document for the author to split
/// the part by.
pub(super) fn drums() -> Patch {
    plain(
        Source::Noise {
            color: NoiseColor::Pink,
        },
        Adsr {
            a: 0.001,
            d: 0.12,
            s: 0.0,
            r: 0.12,
            curve: 0.0,
        },
    )
}

fn plain(source: Source, amp: Adsr) -> Patch {
    Patch {
        layers: Vec::new(),
        source,
        amp,
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![],
    }
}

const SHARPS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const FLATS: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

/// Names a MIDI key the way a score does — `60` is `C4` — spelled with flats
/// when the file's key signature is a flat one, and sharps otherwise.
///
/// The spelling follows the signature because that is the one place the file
/// says how its black keys are written; `A#` all through a piece in F would be
/// right and unreadable.
pub(super) fn spell(key: u8, flats: bool) -> String {
    let names = if flats { FLATS } else { SHARPS };
    format!(
        "{}{}",
        names[usize::from(key % 12)],
        i32::from(key / 12) - 1
    )
}

/// Every major key a signature can name, flattest first: index `sharps + 7`.
const MAJOR: [&str; 15] = [
    "Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#",
];
/// Every minor key a signature can name, the same way round.
const MINOR: [&str; 15] = [
    "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#",
];

/// The key a signature names — `-3, false` is `"Eb major"` — or nothing for a
/// signature past seven accidentals, which no key has.
pub(super) fn key_name(sharps: i8, minor: bool) -> Option<String> {
    let index = usize::try_from(i32::from(sharps) + 7).ok()?;
    let (tonics, mode) = if minor {
        (MINOR, "minor")
    } else {
        (MAJOR, "major")
    };
    tonics.get(index).map(|tonic| format!("{tonic} {mode}"))
}

/// The signature a key is written with — [`key_name`] the other way round —
/// or nothing for a mode a signature cannot say. `written` is the song's
/// `key` as the author spelled it, and `key` the same text read.
///
/// A MIDI key signature knows two modes, major and minor, so `D dorian` has
/// none: writing C major's would claim the piece is in C. The tonic is read
/// as the author spelled it, so `Db major` is five flats; a spelling no
/// signature uses (`D# major`, nine sharps) falls back to the one of the same
/// pitch with the fewest accidentals, which is how a score would write it.
pub(super) fn signature(written: &str, key: &Key) -> Option<(i8, bool)> {
    let minor = match key.mode {
        Mode::Ionian => false,
        Mode::Aeolian => true,
        _ => return None,
    };
    let tonics = if minor { MINOR } else { MAJOR };
    let written = written.split_whitespace().next().unwrap_or_default();
    let index = tonics
        .iter()
        .position(|tonic| *tonic == written)
        .or_else(|| {
            (0..tonics.len())
                .filter(|&index| pitch_class(tonics[index]) == Some(key.tonic))
                .min_by_key(|&index| index.abs_diff(7))
        })?;
    Some((index as i8 - 7, minor))
}

/// The pitch class a tonic's name spells, `0` for C.
fn pitch_class(tonic: &str) -> Option<i32> {
    crate::parse_note(&format!("{tonic}4"))
        .ok()
        .map(|midi| (midi as i32).rem_euclid(12))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_note;
    use crate::song::Key;

    #[test]
    fn every_spelling_reads_back_as_the_key_it_names() {
        for key in 0..=127_u8 {
            for flats in [false, true] {
                let name = spell(key, flats);
                assert_eq!(parse_note(&name).expect("parses"), f32::from(key), "{name}");
            }
        }
    }

    #[test]
    fn every_signature_is_a_key_the_song_can_read() {
        for sharps in -7..=7 {
            for minor in [false, true] {
                let name = key_name(sharps, minor).expect("in range");
                Key::parse(&name).expect(&name);
            }
        }
        assert_eq!(key_name(-3, false).as_deref(), Some("Eb major"));
        assert_eq!(key_name(0, true).as_deref(), Some("A minor"));
        assert_eq!(key_name(8, false), None);
    }

    #[test]
    fn a_signature_reads_back_as_the_key_it_names() {
        for sharps in -7..=7 {
            for minor in [false, true] {
                let name = key_name(sharps, minor).expect("in range");
                let key = Key::parse(&name).expect(&name);
                assert_eq!(signature(&name, &key), Some((sharps, minor)), "{name}");
            }
        }
    }
}
