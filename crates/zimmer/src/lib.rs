//! zimmer: sound from a document. A recipe (a [`Patch`] for one note, a [`Song`] for music)
//! renders to samples and a WAV, a pure function of the document and [`SYNTH_VERSION`]: no I/O,
//! no clock, no unseeded randomness. The caller does the I/O.

#![deny(clippy::unwrap_used)]

pub(crate) mod core;
pub(crate) mod error;
pub(crate) mod fx;
pub(crate) mod hash;
pub mod level;
pub(crate) mod note;
pub mod patch;
pub mod song;
pub(crate) mod stereo;
pub mod survey;
pub mod wav;

pub use core::SAMPLE_RATE;
pub use error::{NoteAt, SynthError};
pub use note::{midi_to_freq, parse_note, Glide, NoteOpts};
pub use patch::Patch;
pub use song::{render_excerpt, render_song, Excerpt, PatchResolver, Song, Span, Window};

// Bumped with any change to what a recipe renders to, so the sound moves only deliberately: the
// fingerprint test fails when it moves and this number did not.
pub const SYNTH_VERSION: u32 = 10;

pub fn bake_note(patch: &Patch, midi: f32, opts: &NoteOpts) -> Result<Bake, SynthError> {
    // Neither sections nor tracks: a one-shot is one gesture played by one voice, so both tables
    // would be the summary printed a second time.
    Ok(Bake::of(
        &core::render_limited(patch, midi, opts)?,
        Vec::new(),
        Vec::new(),
    ))
}

pub fn bake_named_note(patch: &Patch, note: &str, opts: &NoteOpts) -> Result<Bake, SynthError> {
    bake_note(patch, parse_note(note)?, opts)
}

pub fn render_note(patch: &Patch, midi: f32, opts: &NoteOpts) -> Result<Vec<f32>, SynthError> {
    Ok(core::render_note(patch, midi, opts)?.interleaved())
}

pub fn bake_song(song: &Song, resolve: &dyn PatchResolver) -> Result<Bake, SynthError> {
    bake_excerpt(song, resolve, &Excerpt::default())
}

pub fn bake_excerpt(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
) -> Result<Bake, SynthError> {
    bake_excerpt_unless(song, resolve, excerpt, &song::render::never)
        .map(|bake| bake.expect("a bake nobody can stop runs to the end"))
}

pub fn bake_excerpt_unless(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Bake>, SynthError> {
    let mixed = song::render::mix_song(song, resolve, excerpt, stop)?;
    Ok(mixed.map(|mixed| Bake::of(&mixed.master, mixed.sections, mixed.tracks)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bake {
    pub wav: Vec<u8>,
    pub profile: level::Profile,
    pub tracks: Vec<level::Layer>,
}

impl Bake {
    pub fn loudness(&self) -> &level::Loudness {
        self.profile.loudness()
    }

    fn of(samples: &stereo::Stereo, sections: Vec<level::Cut>, tracks: Vec<level::Layer>) -> Self {
        let interleaved = samples.interleaved();
        let mut profiler = level::Profiler::sectioned(stereo::CHANNELS, SAMPLE_RATE, sections);
        profiler.feed(&interleaved);
        Self {
            wav: wav::encode(&interleaved),
            profile: profiler.finish(),
            tracks,
        }
    }
}
