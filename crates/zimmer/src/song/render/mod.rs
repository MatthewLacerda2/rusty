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
/// Test hook: refuses every named patch; only zimmer's own tests use it.
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

/// Test hook: only zimmer's own tests call it.
pub fn render_song(song: &Song, resolve: &dyn PatchResolver) -> Result<Vec<f32>, SynthError> {
    render_excerpt(song, resolve, &Excerpt::default())
}

/// Test hook: only zimmer's own tests call it.
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

/// Asks `stop` before each note it synthesises and before the fold-down; `Ok(None)` if it stopped.
pub(crate) fn mix_song(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Mixdown>, SynthError> {
    let walk = Walk::new(song, resolve, excerpt)?;
    let arrangement_end = walk.arrangement_end();
    // The master starts at the arrangement's length, so a closing rest is not cut.
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
    // After the limiter: `shape` sets the length, then the fades.
    shape(song, &mut master, arrangement_end);
    // Cut last: the limiter and `shape` run on the piece around the window.
    let (from, to) = walk.scope.keep(master.frames());
    master.cut(from, to);
    Ok(Some(Mixdown {
        master,
        tracks,
        sections,
    }))
}
