//! Walk the arrangement, sum the notes, limit the result.

mod mix;
mod walk;

pub(in crate::song) use mix::has_a_room;
use mix::Mix;
use walk::Walk;

use super::excerpt::Excerpt;
use super::shape::{limit, shape};
use super::{sections, Song};
use crate::error::SynthError;
use crate::level::{Cut, Layer};
use crate::patch::Patch;
use crate::stereo::Stereo;

pub trait PatchResolver {
    fn resolve(&self, reference: &str) -> Result<Patch, String>;
}

impl<F> PatchResolver for F
where
    F: Fn(&str) -> Result<Patch, String>,
{
    fn resolve(&self, reference: &str) -> Result<Patch, String> {
        self(reference)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InlineOnly;

impl PatchResolver for InlineOnly {
    fn resolve(&self, _reference: &str) -> Result<Patch, String> {
        Err("this song must carry its patches inline".to_owned())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mixdown {
    pub(crate) master: Stereo,
    pub(crate) tracks: Vec<Layer>,
    pub(crate) sections: Vec<Cut>,
}

pub fn render_song(song: &Song, resolve: &dyn PatchResolver) -> Result<Vec<f32>, SynthError> {
    render_excerpt(song, resolve, &Excerpt::default())
}

pub fn render_excerpt(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
) -> Result<Vec<f32>, SynthError> {
    let mixed = mix_song(song, resolve, excerpt, &never)?;
    Ok(mixed
        .expect("a render nobody can stop runs to the end")
        .master
        .interleaved())
}

pub(crate) fn never() -> bool {
    false
}

pub(crate) fn mix_song(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Mixdown>, SynthError> {
    let walk = Walk::new(song, resolve, excerpt)?;
    let arrangement_end = walk.arrangement_end();
    let mut mix = Mix::new(song, arrangement_end, &walk.clock, &walk.scope);
    if !walk.run(&mut mix, stop)? {
        return Ok(None);
    }
    let sections = sections::of(song, walk.scope.opens_at_seconds());
    if stop() {
        return Ok(None);
    }
    let (mut master, tracks) = mix.finish(&sections);
    limit(song, &mut master, arrangement_end)?;
    // Then length and level, in that order, on the limited signal.
    shape(song, &mut master, arrangement_end);
    let (from, to) = walk.scope.keep(master.frames());
    master.cut(from, to);
    Ok(Some(Mixdown {
        master,
        tracks,
        sections,
    }))
}
