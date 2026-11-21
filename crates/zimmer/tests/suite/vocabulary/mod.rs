//! `docs/api/Sound.md` is where a game developer reads the recipe vocabulary, so every
//! key a patch or song document takes has a table row there, and every word a key takes
//! (a wave, an easing, a chord quality, a mode) is written in a table. A field, a
//! variant or a document type added without its line on the page fails here.

mod page;
mod probe;
mod tables;

use probe::{keys, tags, words};
use rusty_zimmer::patch::{
    self, Adsr, Algorithm, EqBand, EqKind, FilterKind, Fx, LfoShape, LfoTarget, NoiseColor, Osc,
    Partial, PitchEnv, Slope, Source, Wave,
};
use rusty_zimmer::song::{
    Anchor, Arp, Articulation, Automation, Chord, DegreeNote, Easing, Fade, Fit, FitMode, FitTo,
    Humanize, Layers, Leading, Param, Play, Point, Steps, Tail, TempoChange,
};
use rusty_zimmer::{Patch, Song};

/// The document types whose keys are their own, by the name the source declares them
/// under, with the keys each takes.
fn documents() -> Vec<(&'static str, Vec<String>)> {
    let mut found = vec![
        ("Patch", keys::<Patch>("")),
        ("Osc", keys::<Osc>("")),
        ("Partial", keys::<Partial>("")),
        ("Adsr", keys::<Adsr>("")),
        ("PitchEnv", keys::<PitchEnv>("")),
        ("Filter", keys::<patch::Filter>("")),
        ("Operator", keys::<patch::Operator>("")),
        ("Layer", keys::<patch::Layer>("")),
        ("Lfo", keys::<patch::Lfo>("")),
        ("EqBand", keys::<EqBand>("")),
        ("Song", keys::<Song>("")),
        ("Track", keys::<rusty_zimmer::song::Track>("")),
        ("Pattern", keys::<rusty_zimmer::song::Pattern>("")),
        ("Note", keys::<rusty_zimmer::song::Note>("")),
        ("DegreeNote", keys::<DegreeNote>("")),
        ("Chord", keys::<Chord>("")),
        ("Steps", keys::<Steps>("")),
        ("Play", keys::<Play>("")),
        ("Layers", keys::<Layers>("")),
        ("Automation", keys::<Automation>("")),
        ("Point", keys::<Point>("")),
        ("TempoChange", keys::<TempoChange>("")),
        ("Fit", keys::<Fit>("")),
        ("Fade", keys::<Fade>("")),
        ("Anchor", keys::<Anchor>("")),
        ("Humanize", keys::<Humanize>("")),
    ];
    found.push(("Source", probe::tagged::<Source>("kind")));
    found.push(("Fx", probe::tagged::<Fx>("fx")));
    found
}

/// The word-valued types, with the words each takes.
fn vocabularies() -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("Source", tags::<Source>("kind")),
        ("Fx", tags::<Fx>("fx")),
        ("Wave", words::<Wave>()),
        ("NoiseColor", words::<NoiseColor>()),
        ("FilterKind", words::<FilterKind>()),
        ("Slope", words::<Slope>()),
        ("Algorithm", words::<Algorithm>()),
        ("EqKind", words::<EqKind>()),
        ("LfoTarget", words::<LfoTarget>()),
        ("LfoShape", words::<LfoShape>()),
        ("Param", words::<Param>()),
        ("Easing", words::<Easing>()),
        ("FitTo", words::<FitTo>()),
        ("FitMode", words::<FitMode>()),
        ("Tail", words::<Tail>()),
        ("Arp", words::<Arp>()),
        ("Leading", words::<Leading>()),
        ("Articulation", words::<Articulation>()),
        ("chord qualities", tables::qualities()),
        ("key modes", tables::modes()),
    ]
}

#[test]
fn every_key_and_word_a_recipe_takes_is_on_the_page() {
    let page = page::read();
    let mut missing = Vec::new();
    for (owner, taken) in documents() {
        let absent = taken.iter().filter(|key| !page.rows.contains(*key));
        missing.extend(absent.map(|key| format!("key `{key}` of {owner} has no table row")));
    }
    for (owner, taken) in vocabularies() {
        let absent = taken.iter().filter(|word| !page.words.contains(*word));
        missing.extend(absent.map(|word| format!("word `{word}` of {owner} is in no table")));
    }
    assert!(
        missing.is_empty(),
        "docs/api/Sound.md does not document the whole recipe vocabulary; give each a \
         table row (key, default, unit or range, meaning):\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn every_document_type_is_probed() {
    let probed: Vec<&str> = documents()
        .iter()
        .chain(&vocabularies())
        .map(|(name, _)| *name)
        .collect();
    let unprobed: Vec<String> = tables::deserialized()
        .into_iter()
        .filter(|name| {
            !probed.contains(&name.as_str()) && !tables::KEYLESS.contains(&name.as_str())
        })
        .collect();
    assert!(
        unprobed.is_empty(),
        "these document types are read from a recipe but `vocabulary` never asks the page \
         about them; add each to `documents` or `vocabularies` (or to `tables::KEYLESS` \
         if it only chooses between other types): {unprobed:?}"
    );
}
