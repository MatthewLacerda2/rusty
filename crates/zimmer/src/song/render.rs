//! Walk the arrangement, sum the notes, limit the result.
//!
//! The whole renderer is one idea: **mixing is addition**. Every note is
//! rendered on its own through the note renderer — independently of the
//! others, with the single exception stated below — and its buffer is added
//! into the master at its start offset. There is no streaming, no voice
//! allocation and no voice-stealing, because none of that buys anything here —
//! this is not a real-time synth, it is a buffer being built, and memory is
//! cheap.
//!
//! **One note is not independent of the others, and it is the exception this
//! paragraph exists to state.** A note marked `glide` starts at the pitch of
//! the note before it on its track, so the loop below carries a per-track
//! *hand* — where that line has got to — rather than reading each note alone.
//! Nothing else about a note asks that question, and nothing else should:
//! independence is what makes the rest of this file a sum. What "the note
//! before it" means is decided here rather than left to fall out of the shape
//! of the document, and [`super::glide`] is where it is worked out:
//!
//! - **The note that last *started* on the track**, in time — not the one
//!   written above it in the file. The loop walks a pattern's entries in
//!   document order, which is not time order, and reordering a `notes` array
//!   changes no music today. A glide must not be the first thing that makes
//!   it: a slide is a hand moving from where it was, and where it was is a
//!   fact about the clock.
//! - **Notes that start together are one moment.** A chord's voices, or two
//!   notes written on one beat, each slide from whatever preceded the moment
//!   rather than from each other, and the line goes on from the **highest** of
//!   them, which is the voice an ear follows. A *block* chord marked `glide`
//!   therefore arrives from one pitch and opens out into its voicing — a real
//!   gesture, but not the parallel slide a guitarist means, and a document
//!   that wants that writes the voices as separate notes. An
//!   [`Arp`](super::Arp) is not this case at all, because its voices land at
//!   different onsets: each slides from the one before it, and the figure
//!   crawls up the chord instead of jumping about in it.
//! - **A note that did not sound was still played.** A `tracks` filter and a
//!   solo skip notes without spending the score, exactly as they spend an
//!   ordinal without playing one. Two things need that: muting eight bars must
//!   not change how the ninth is *played*, and an excerpt promises the notes
//!   it keeps sound as they do in the whole piece — which a hand that only
//!   moved on what the window rendered would break.
//! - **Nothing is conditional on the gap.** A glide slides from the previous
//!   note however long ago it stopped. Legato mode on a mono synth is
//!   conditional, and copying it would put the meaning of one note's mark in
//!   another note's `dur`: shortening the note before it, or marking that one
//!   `staccato`, would silently turn the slide off. The mark says slide. Only
//!   the first note of a track has nothing to slide from, and that one is
//!   played plain.
//!
//! Two deliberate consequences:
//!
//! - **Nothing is cut off.** The buffer grows to fit whatever the last note's
//!   release and fx tail need, so a song ends by ringing out rather than by a
//!   hard stop at the final beat.
//! - **Notes are rendered *unlimited*** and only the finished mix is limited.
//!   The per-note limiter exists so a single baked one-shot cannot clip its own
//!   file; applying it here as well would squash each note's dynamics *before*
//!   the mix, then squash the sum again. The master limiter is the guarantee
//!   that matters, and it is not optional.
//!
//! This file walks the arrangement and hands each rendered note to the `mix`
//! module beside it, which owns where a note lands and what runs on the sums —
//! a chain on a track, or one on the whole piece. Those two stages are the
//! paragraph above one level up: some things belong to the sum and not to the
//! parts, which is also why a song's chain runs *before* the limiter here and
//! never after it.
//!
//! **Determinism.** Every note's seed is derived from `(song.seed, track
//! index, note ordinal)` through the same seeded integer hash everything else
//! here uses — no `rand`, no wall clock. The ordinal counts notes in
//! arrangement order, so a pattern played twice gets two different noise draws
//! (a repeated snare should not be a photocopy) while the whole piece stays
//! byte-identical across runs and processes. [`super::feel`] draws its onset,
//! velocity and timbre nudges from the same coordinates, so humanising a song
//! puts no asterisk on any of that — and neither does the note renderer
//! starting each oscillator somewhere in its cycle, which reads the same seed.

use std::collections::HashMap;

use super::articulation::Stroke;
use super::automate::{self, Automation};
use super::clock::Clock;
use super::excerpt::{Excerpt, Scope};
use super::feel::{Played, swung};
use super::glide::{Slides, Trail};
use super::mix::Mix;
use super::shape::{limit, plan, shape};
use super::{Articulation, Note, PatchRef, Song, sections};
use crate::core::lfo::Onset;
use crate::core::{self, RATE};
use crate::error::{NoteAt, SynthError};
use crate::hash::hash3;
use crate::level::{Cut, Layer};
use crate::note::{Glide, NoteOpts};
use crate::patch::Patch;
use crate::stereo::Stereo;

/// Supplies the patch behind a track that names its instrument rather than
/// carrying it inline.
///
/// A trait rather than a path, because this crate does no I/O and has never
/// heard of a project. The caller decides what a name means and whether it is
/// allowed to be read — in scorsese, that a recipe stays inside the project
/// root.
///
/// The failure is text because only the caller knows what went wrong; it is
/// wrapped in [`SynthError::UnresolvedPatch`] with the track and reference that
/// asked for it.
pub trait PatchResolver {
    /// Produces the patch this reference names, or says why it cannot.
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

/// A resolver that refuses every reference — for songs that carry all their
/// instruments inline, where being asked to resolve one at all is the bug.
#[derive(Debug, Clone, Copy, Default)]
pub struct InlineOnly;

impl PatchResolver for InlineOnly {
    fn resolve(&self, _reference: &str) -> Result<Patch, String> {
        Err("this song must carry its patches inline".to_owned())
    }
}

/// A rendered song, and what each of its tracks put into it.
///
/// The two arrive together because the second only exists while the first is
/// being made: once the mix is summed there is no way back to the parts, and
/// re-rendering a track alone to measure it would be paying twice for a number
/// that was already in hand.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mixdown {
    /// The finished stereo buffer — the thing that gets encoded.
    pub(crate) master: Stereo,
    /// One row per track, in track order, **post-gain**: what that instrument
    /// takes up in the mix rather than what it would sound like alone.
    ///
    /// The song's own chain, the master limiter and the fades are *not* in
    /// these numbers. Those belong to the sum, and a row that included them
    /// would be answering a question about the piece under the name of a
    /// track.
    ///
    /// **Empty for a song of fewer than two tracks**, which is the rule
    /// [`crate::level::Profile`] already holds sections to: one row under a
    /// one-line summary is the same sentence twice.
    pub(crate) tracks: Vec<Layer>,
    /// Where the arrangement's own sections fall in what came back, which
    /// under an excerpt is not where they fall in the piece.
    ///
    /// Handed out with the samples rather than worked out again beside them,
    /// because the tempo and pass count they are measured against are decided
    /// here — and under a `fit` they are not the ones written down.
    pub(crate) sections: Vec<Cut>,
}

/// Renders `song` to an interleaved stereo sample buffer at
/// [`crate::SAMPLE_RATE`], master-limited, and the length the song asks to be.
///
/// Interleaved — left sample first — because that is the form a WAV holds and
/// the form [`crate::level`] measures, so a caller doing anything at all with
/// raw samples already speaks it.
pub fn render_song(song: &Song, resolve: &dyn PatchResolver) -> Result<Vec<f32>, SynthError> {
    render_excerpt(song, resolve, &Excerpt::default())
}

/// [`render_song`], of less of the song: a stretch of it, some of its tracks,
/// or both. [`Excerpt`] has what that means and what it promises.
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

/// The "should stop" of a render nobody can stop.
pub(crate) fn never() -> bool {
    false
}

/// [`render_song`], keeping what each track contributed on its way into the
/// mix — see [`Mixdown::tracks`].
///
/// `stop` is asked before each note is synthesised and once more before the
/// sums are folded down, and `Ok(None)` is what a render it stopped comes back
/// with (#661). Before each note because a note is the unit of work here — the
/// buffer each one renders is the cost, and everything between two of them is
/// arithmetic — so a stop is honoured within one note's time. Asking it is the
/// whole cost of the hook, and it changes no sample: a render that is never
/// stopped is the render it always was.
pub(crate) fn mix_song(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Mixdown>, SynthError> {
    song.validate()?;
    let patches = resolve_patches(song, resolve)?;
    // The one check that needs an instrument rather than a document: a cutoff
    // curve on a patch with no filter moves nothing, and a curve that moves
    // nothing is the failure automation is validated against.
    super::validate::check_resolved(song, &patches)?;
    // Which curves ride which track, looked up once rather than per note.
    let riding = automate::riding(song);
    // How fast to play it and how many times through, worked out before a
    // sample is produced: `fit` is a property of the whole piece, and deciding
    // it per note would mean rendering the wrong notes and cutting afterwards.
    // The clock is the one conversion from beats to seconds, shared with the
    // report's section rows so a boundary is said exactly where the notes
    // were put — and it carries the song's tempo map, if it writes one.
    let (clock, passes) = plan(song);
    // What less of this piece was asked for, resolved against the tempo it is
    // actually rendered at. A whole render resolves to one that keeps
    // everything, so there is one path below rather than two.
    let scope = Scope::of(song, excerpt, &clock)?;
    // Read once: every degree in the song resolves against it, and so does
    // every diatonic lift in the arrangement.
    let key = song.key()?;
    // Every written form becomes ordinary notes here — a chord its voices, a
    // step string its hits, a degree its pitch — once per pattern and before a
    // sample is produced. So everything below this line sees notes and nothing
    // else, and each of them picks up its own ordinal, its own swing
    // displacement and its own humanise nudge, rather than a chord landing as
    // one rigid block or a hi-hat part as one photocopied strike.
    let voiced: HashMap<&str, Vec<Note>> = song
        .patterns
        .iter()
        .map(|(name, pattern)| Ok((name.as_str(), pattern.voices(key.as_ref())?)))
        .collect::<Result<_, SynthError>>()?;
    let track_index: HashMap<&str, usize> = song
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| (track.name.as_str(), index))
        .collect();
    // Who each note slides from — worked out per pattern, and only for a song
    // that writes the one mark that asks. A piece with no glide in it takes
    // exactly the walk it took before this existed, resolving no pitch it has
    // no reason to look at; the empty map below is what says so.
    let slides: HashMap<&str, Slides> = if glides(&voiced) {
        voiced
            .iter()
            .map(|(name, notes)| Ok((*name, Slides::of(notes, &track_index, song.tracks.len())?)))
            .collect::<Result<_, SynthError>>()?
    } else {
        HashMap::new()
    };
    let mut trail = Trail::new(song.tracks.len());

    // Start at the arrangement's own length rather than growing from empty, so
    // a song whose last pattern ends on a rest keeps that rest. Truncating to
    // the final *note* instead would silently shorten the buffer and put a loop
    // point in the wrong place — the arrangement is the score, not just where
    // the samples happen to stop. Tails then extend it past this.
    let played = song.arrangement_beats() * passes as f32;
    let arrangement_end = (clock.seconds(played) * RATE).round() as usize;
    // The clock this is actually rendered at — the stretched one under a
    // `stretch` fit, which is what makes a build stretch with the music
    // instead of landing somewhere else in it.
    let mut mix = Mix::new(song, arrangement_end, &clock, &scope);
    let mut cursor_beats = 0.0f32;
    let mut ordinal: u64 = 0;
    // Resolved once per track: an absent `humanize` is one that scatters
    // nothing, and a track that says nothing of its feel plays the song's, so
    // the note loop has a single path rather than a branch per note.
    let played: Vec<Played> = song.tracks.iter().map(|it| Played::of(song, it)).collect();

    // Repeats are a longer arrangement, not a copied buffer: tiling rendered
    // audio would overlap each pass's ring-out onto the next pass's downbeat,
    // and would replay the same seeded noise every time round.
    for entry in song
        .arrangement
        .iter()
        .cycle()
        .take(song.arrangement.len() * passes as usize)
    {
        // Every layer starts on the slot's first beat, in the order written — an
        // entry that is not layered is one layer, walked exactly as it always
        // was, which is what keeps every song from before layers byte-identical.
        for layer in entry.layers() {
            if !song.patterns.contains_key(layer.pattern()) {
                return Err(SynthError::UnknownPattern {
                    pattern: layer.pattern().to_owned(),
                });
            }
            let sliding = slides.get(layer.pattern());
            for (index, note) in voiced
                .get(layer.pattern())
                .into_iter()
                .flatten()
                .enumerate()
            {
                // A silenced track still consumes its ordinal, so muting one for
                // eight bars does not re-roll the noise of every note after it.
                let track = track_index[note.track.as_str()];
                let Played {
                    swing,
                    humanize: feel,
                    push,
                } = played[track];
                let place = ordinal;
                let seed = note_seed(song.seed, track, place);
                ordinal += 1;
                if !layer.plays(&note.track) {
                    continue;
                }
                // A track a solo left out, and that nothing is keyed from, is not
                // rendered at all — which is where a solo's saving is. The ordinal
                // above it has already been spent, so what is left of the mix
                // sounds exactly as it does in the whole piece.
                if !mix.needs(track) {
                    continue;
                }
                // The layer's transforms, applied to the *written* pattern rather
                // than to whatever the previous entry produced: they do not stack
                // across entries, nor onto the layer sounding beside it, so the tenth repeat is not nine octaves up and
                // the document still says what it does.
                let stroke = Stroke::of(note.articulation);
                // Velocity, in the order the three things that scale it were
                // decided: what the page wrote, then what the section does to the
                // whole pattern (`vel_scale`), then what the mark over this one
                // note does (an accent, a ghost), and only then the player's own
                // inaccuracy on top of all three. Humanise is last because it is
                // the error term: it scatters a decision and never overrules one.
                let written = note.vel * layer.vel_scale() * stroke.velocity;
                let velocity = feel.velocity(written, track, place, song.seed);
                // Where this note sits in the piece, in beats: the one coordinate
                // an automation curve is read at, and the one its length is
                // measured from once the tempo moves. Swung, because that is where
                // the note is actually played — but neither marked nor humanised,
                // since a curve read at a displaced onset would put a ghost's
                // earliness and the player's jitter on the build as well as on the
                // note.
                let beat_at = cursor_beats + swung(note.start, swing);
                // The gate, not the written `dur`: staccato and ghost shorten how
                // long the note is held and leave the rhythm on the page exactly
                // as it reads. Measured from where it is played, because a beat
                // in a ritardando lasts longer than one before it.
                let gate = clock.span(beat_at, note.dur * stroke.gate);
                // Both transposes, applied in one place — and clamped rather than
                // refused, since refusing would make a legal transpose depend on
                // the register of a pattern written months ago.
                let pitch = layer.played_pitch(note.note.to_midi()?, key.as_ref());
                // Where the hand was. A mark that found nothing to slide from —
                // the first note of a track — is played plain rather than
                // refused: what a glide needs is another note, and the top of a
                // piece has not got one yet.
                let glide = match (stroke.slide_seconds(gate), sliding) {
                    (Some(seconds), Some(slides)) => trail
                        .from(slides, index, track, layer, key.as_ref())
                        .map(|from| Glide {
                            semitones: from - pitch,
                            seconds,
                        }),
                    _ => None,
                };
                let opts = NoteOpts {
                    duration: gate,
                    velocity,
                    // How far this strike's tone sits from its level — the mark's
                    // own offset plus the player's. Both are fractions of the
                    // velocity actually played, in the same units against the same
                    // number, which is what lets them simply add: intent first,
                    // then the error on it.
                    timbre: stroke.timbre(velocity)
                        + feel.timbre(velocity, track, place, song.seed),
                    glide,
                    seed,
                };
                // Swing first, then the mark, then humanise, then the track's
                // push: swing is where the beat *is*, an articulation is where
                // the player meant to put the note against it (a ghost sits a
                // hair ahead), humanise is how well they hit what they meant, and
                // the push is where this part sits against everyone else's beat.
                // The last three are seconds added to the same number, so the
                // order is what they mean rather than what the arithmetic needs. Clamped at zero rather than wrapped — a note
                // nudged early on the very first beat has nowhere to go, and a
                // negative sample index is not a time.
                let onset = clock.seconds(beat_at)
                    + stroke.onset_seconds
                    + feel.onset_seconds(track, place, song.seed)
                    + push;
                let at = (onset * RATE).round().max(0.0) as usize;
                // Worked out before the note is synthesised rather than after,
                // because a note landing past what a window can hear is the one
                // this loop wants to *not* pay for. Everything above it is
                // arithmetic; `render_note` is the buffer.
                if !scope.reaches(at) {
                    continue;
                }
                if stop() {
                    return Ok(None);
                }
                let instrument = &patches[track];
                let swept = riding[track]
                    .cutoff
                    .zip(instrument.filter)
                    .map(|(curve, filter)| sweep(curve, &clock, beat_at, filter.cutoff));
                // A note too long to render is only found here, where its
                // gate is known in seconds; the refusal is told where the song
                // writes it, since "a note" is no help in a piece of hundreds.
                // Where the note sounds, so an LFO timed in beats keeps its
                // phase on the song's grid rather than the note's.
                let onset = Onset {
                    grid: &clock,
                    frame: at,
                };
                let rendered = core::render_swept(
                    instrument,
                    pitch,
                    &opts,
                    swept.as_ref().map(|sweep| sweep as &dyn Fn(usize) -> f32),
                    Some(&onset),
                )
                .map_err(|error| {
                    error.at(NoteAt {
                        pattern: layer.pattern().to_owned(),
                        track: note.track.clone(),
                        start: note.start,
                    })
                })?;
                // Added to the track's own bus rather than straight to the master:
                // where a note lands is timing, which bus it lands on is routing,
                // and the two answer to different fields.
                mix.add(track, &rendered, at);
            }
        }
        // Every hand moves to where this playing left it, whether or not the
        // notes that moved it were allowed to sound — see the module doc. Only
        // once the whole slot is walked, so a layer slides from where the
        // previous slot left a hand and never from a layer sounding beside it.
        for layer in entry.layers() {
            if let Some(slides) = slides.get(layer.pattern()) {
                trail.advance(slides, layer, key.as_ref());
            }
        }
        // Every layer's pattern was found above, so the slot is known.
        cursor_beats += song.slot_beats(entry).unwrap_or_default();
    }

    // Track buses folded down and the song's own chain applied — everything
    // that belongs to a sum rather than to a note. Each track is measured as it
    // goes past, which is the only moment it exists on its own.
    // Where the arrangement's sections fall in what comes back, worked out
    // once: the tracks are cut at them as they are measured, and the sum is
    // cut at them by the caller, and the two tables only line up if both are
    // cut at the same numbers.
    let sections = sections::of(song, scope.opens_at_seconds());
    if stop() {
        return Ok(None);
    }
    let (mut master, tracks) = mix.finish(&sections);
    // The master limiter, always — mixing by addition is exactly the operation
    // that overshoots full scale, so the sum is never handed out unlimited.
    // A `wrap` tail folds the ring-out onto the start first, because that
    // fold is one more addition.
    limit(song, &mut master, arrangement_end)?;
    // Then length and level, in that order, on the limited signal.
    shape(song, &mut master, arrangement_end);
    // And only now is the window taken, which is what makes it exactly the
    // stretch a whole render would have had there: every stage above ran on
    // the piece, not on the excerpt.
    let (from, to) = scope.keep(master.frames());
    master.cut(from, to);
    Ok(Some(Mixdown {
        master,
        tracks,
        sections,
    }))
}

/// Where a `cutoff` curve puts the filter at each sample-frame of a note
/// struck on `beat`, counted from its onset — or `fallback`, the patch's own
/// cutoff, should the curve have no points to read.
///
/// Read across the note rather than once at its onset (#1123): a pad held
/// through an eight-bar build opens with it, where an onset reading held the
/// whole chord at the value its first sample saw, and a smooth riser had to be
/// written as noise re-struck every 32nd to sound like one. The note's frames
/// are turned back into beats by the same clock that placed it, so a sweep
/// under a ritardando or a `fit` follows the beats it was written on.
fn sweep<'a>(
    curve: &'a Automation,
    clock: &'a Clock,
    beat: f32,
    fallback: f32,
) -> impl Fn(usize) -> f32 + 'a {
    let onset = (clock.seconds(beat) * RATE).round().max(0.0) as usize;
    move |frame| {
        curve
            .value_at(clock.beat_at(onset + frame))
            .unwrap_or(fallback)
    }
}

/// Whether any note in the piece is slid onto — the one question that decides
/// whether a render has to know what came before a note.
///
/// Asked of the *voiced* notes rather than of the written entries, so it costs
/// one pass over notes that have already been expanded and cannot disagree
/// with what the loop below sees.
fn glides(voiced: &HashMap<&str, Vec<Note>>) -> bool {
    voiced
        .values()
        .flatten()
        .any(|note| note.articulation == Some(Articulation::Glide))
}

/// One seed per note, from `(song seed, track, ordinal)`.
///
/// Two 32-bit hashes on different channels are stitched into the `u64` the note
/// renderer wants, so the full seed space is used rather than the low 32 bits.
///
/// Those are channels **0 and 1** of these coordinates; [`super::feel`] draws
/// its onset, velocity and timbre nudges from 2, 3 and 4 of the same pair,
/// which is what keeps a note's timing, its loudness, its tone and its noise
/// from moving together.
fn note_seed(song_seed: u64, track: usize, ordinal: u64) -> u64 {
    let hi = u64::from(hash3(track as i64, ordinal as i64, 0, song_seed));
    let lo = u64::from(hash3(track as i64, ordinal as i64, 1, song_seed));
    (hi << 32) | lo
}

/// Resolves every track's patch once, up front.
///
/// Up front rather than per note because a song is thousands of notes over a
/// handful of instruments. Failing here also means a missing instrument is
/// reported before any rendering happens.
fn resolve_patches(song: &Song, resolve: &dyn PatchResolver) -> Result<Vec<Patch>, SynthError> {
    song.tracks
        .iter()
        .map(|track| match &track.patch {
            PatchRef::Inline(patch) => Ok((**patch).clone()),
            PatchRef::Named(reference) => {
                resolve
                    .resolve(reference)
                    .map_err(|reason| SynthError::UnresolvedPatch {
                        track: track.name.clone(),
                        reference: reference.clone(),
                        reason,
                    })
            }
        })
        .collect()
}
