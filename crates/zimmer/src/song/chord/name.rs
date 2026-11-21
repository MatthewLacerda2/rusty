//! What a chord name spells: a root, and the semitones stacked on it.

use crate::error::SynthError;

const NATURALS: [(char, i32); 7] = [
    ('C', 0),
    ('D', 2),
    ('E', 4),
    ('F', 5),
    ('G', 7),
    ('A', 9),
    ('B', 11),
];

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

pub(crate) struct Spelling {
    pub(crate) root: i32,
    pub(crate) offsets: Vec<i32>,
}

pub(crate) fn spell(name: &str) -> Result<Spelling, SynthError> {
    let unknown = || SynthError::UnknownChord {
        chord: name.to_owned(),
    };
    // The bass is split off first, so the quality lookup never sees a `/` and can never invent a
    // quality out of one.
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
        offsets.push((class - root).rem_euclid(12) - 12);
    }
    offsets.extend_from_slice(stack);
    Ok(Spelling { root, offsets })
}

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

    #[test]
    fn a_slash_bass_goes_underneath() {
        let spelled = spell("C/G").expect("a slash chord");
        assert_eq!(spelled.offsets, vec![-5, 0, 4, 7]);
        // The root named as its own bass is that root doubled an octave down.
        assert_eq!(spell("C/C").expect("legal").offsets, vec![-12, 0, 4, 7]);
    }

    #[test]
    fn anything_off_the_table_is_refused() {
        for name in [
            "", "H", "dm7", "CM7", "Cmaj13", "C11", "Cxyz", "C/", "C/H", "C/Gm", "Cs4", "C ",
        ] {
            assert!(spell(name).is_err(), "`{name}` should not have spelled");
        }
    }
}
