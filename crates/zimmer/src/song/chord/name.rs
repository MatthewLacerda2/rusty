//! What a chord name spells: a root, and the semitones stacked on it.
//!
//! The grammar is `root [quality] [/bass]` and it is **closed**. A name is
//! looked up in the table below or it is refused — nothing here infers a
//! quality from a suffix it half-recognises, and nothing falls back to a major
//! triad. That is the whole point of the module: a chord that silently means
//! something other than what was written is worse than the four notes it
//! replaced, because four wrong notes are visible in the document and a misread
//! name is not.
//!
//! **The root is spelled the way a note is, minus the octave**, with two
//! deliberate narrowings from [`parse_note`](crate::parse_note):
//!
//! - the letter must be **upper case**, so `bb` cannot be read as either
//!   "B flat" or "the flat five of B" depending on who is looking;
//! - accidentals are `#` and `b` only — `s` and `f` are legal in a note name
//!   and would collide here with `sus` and with the quality letters.
//!
//! Accidentals are consumed greedily, which is the ordinary chart reading:
//! `Bb7` is B-flat dominant seventh, and a flat five is written `C7b5`, where
//! the `b` sits after a quality rather than after the letter.
//!
//! **Elevenths and thirteenths are absent on purpose.** Both are named for a
//! tone they add and played by omitting others — a `C13` with all seven of its
//! notes is mud, and which ones to drop is a judgement. A judgement is exactly
//! what a closed table must not make on the writer's behalf, so those are the
//! chords to spell out as pitches. The same reasoning excludes `M` for major:
//! it differs from `m` by one shift key and means the opposite.

use crate::error::SynthError;

/// Semitone offset of each natural, upper case only — see the module doc.
const NATURALS: [(char, i32); 7] = [
    ('C', 0),
    ('D', 2),
    ('E', 4),
    ('F', 5),
    ('G', 7),
    ('A', 9),
    ('B', 11),
];

/// Every quality this synthesiser knows, and the semitones it stacks on the
/// root — ascending, root first.
///
/// A flat list rather than a small algebra of "quality plus alterations":
/// twenty-eight rows can be read and argued with, whereas a rule that composed
/// suffixes would accept names nobody meant and produce chords nobody chose.
const QUALITIES: &[(&str, &[i32])] = &[
    ("", &[0, 4, 7]),
    ("maj", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("min", &[0, 3, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("5", &[0, 7]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("min7", &[0, 3, 7, 10]),
    ("mmaj7", &[0, 3, 7, 11]),
    ("m7b5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("aug7", &[0, 4, 8, 10]),
    ("7sus4", &[0, 5, 7, 10]),
    ("7b5", &[0, 4, 6, 10]),
    ("7b9", &[0, 4, 7, 10, 13]),
    ("7#9", &[0, 4, 7, 10, 15]),
    ("add9", &[0, 4, 7, 14]),
    ("madd9", &[0, 3, 7, 14]),
    ("9", &[0, 4, 7, 10, 14]),
    ("maj9", &[0, 4, 7, 11, 14]),
    ("m9", &[0, 3, 7, 10, 14]),
];

/// What a name spells: the root's pitch class, and the semitones from that
/// root that sound — ascending, and negative for a slash bass under it.
pub(crate) struct Spelling {
    /// The root's pitch class, `0` for C.
    pub(crate) root: i32,
    /// Semitones from the root, ascending.
    pub(crate) offsets: Vec<i32>,
}

/// Reads a chord name, or refuses it.
///
/// Refusal is the only alternative to a correct reading — see the module doc.
pub(crate) fn spell(name: &str) -> Result<Spelling, SynthError> {
    let unknown = || SynthError::UnknownChord {
        chord: name.to_owned(),
    };
    // The bass is split off first, so the quality lookup never sees a `/` and
    // can never invent a quality out of one.
    let (chord, bass) = match name.split_once('/') {
        Some((chord, bass)) => (chord, Some(bass)),
        None => (name, None),
    };
    let (root, quality) = split_root(chord).ok_or_else(unknown)?;
    let stack = QUALITIES
        .iter()
        .find(|(spelled, _)| *spelled == quality)
        .map(|(_, stack)| *stack)
        .ok_or_else(unknown)?;

    let mut offsets = Vec::with_capacity(stack.len() + 1);
    if let Some(bass) = bass {
        let (class, rest) = split_root(bass).ok_or_else(unknown)?;
        if !rest.is_empty() {
            return Err(unknown());
        }
        // The named bass in the octave below the root: `C/G` is a G under a C
        // and never a G above it, because that is what the notation means and
        // "below" is the whole reason anybody writes one. A bass naming the
        // root doubles it an octave down, which is a voicing people ask for.
        offsets.push((class - root).rem_euclid(12) - 12);
    }
    offsets.extend_from_slice(stack);
    Ok(Spelling { root, offsets })
}

/// Splits the root off the front: its pitch class, and whatever follows.
///
/// `None` when the first character is not a natural — there is no root, so
/// there is no chord.
///
/// The single place text becomes a root, which is why
/// [`Key::parse`](crate::song::Key::parse) reads its tonic through it too: a
/// key's tonic is spelled exactly the way a chord's root is, and two copies of
/// the rule would be two places for `Bb` to mean different things.
pub(crate) fn split_root(text: &str) -> Option<(i32, &str)> {
    let mut chars = text.chars();
    let letter = chars.next()?;
    let natural = NATURALS
        .iter()
        .find(|(name, _)| *name == letter)
        .map(|(_, semitone)| *semitone)?;
    let rest = chars.as_str();
    let mut offset = 0;
    let mut end = 0;
    for (index, character) in rest.char_indices() {
        match character {
            '#' => offset += 1,
            'b' => offset -= 1,
            _ => break,
        }
        end = index + character.len_utf8();
    }
    Some(((natural + offset).rem_euclid(12), &rest[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spot-checks across the table, including the two names the accidental
    /// rule exists for.
    #[test]
    fn the_names_spell_what_a_chart_means_by_them() {
        for (name, root, offsets) in [
            ("C", 0, vec![0, 4, 7]),
            ("Dm7", 2, vec![0, 3, 7, 10]),
            ("Bb7", 10, vec![0, 4, 7, 10]),
            ("F#maj9", 6, vec![0, 4, 7, 11, 14]),
            ("Absus4", 8, vec![0, 5, 7]),
            ("C7b5", 0, vec![0, 4, 6, 10]),
        ] {
            let spelled = spell(name).expect("a name from the table");
            assert_eq!((spelled.root, spelled.offsets), (root, offsets), "{name}");
        }
    }

    /// A slash bass sits below the root, and the offsets stay ascending so the
    /// voices come out low to high.
    #[test]
    fn a_slash_bass_goes_underneath() {
        let spelled = spell("C/G").expect("a slash chord");
        assert_eq!(spelled.offsets, vec![-5, 0, 4, 7]);
        // The root named as its own bass is that root doubled an octave down.
        assert_eq!(spell("C/C").expect("legal").offsets, vec![-12, 0, 4, 7]);
    }

    /// The whole reason the table is closed: nothing is guessed.
    #[test]
    fn anything_off_the_table_is_refused() {
        for name in [
            "", "H", "dm7", "CM7", "Cmaj13", "C11", "Cxyz", "C/", "C/H", "C/Gm", "Cs4", "C ",
        ] {
            assert!(spell(name).is_err(), "`{name}` should not have spelled");
        }
    }
}
