//! Where the notes are summed, and where an effect that belongs to the *mix*
//! runs rather than to one instrument.
//!
//! [`crate::patch::Patch`] carries an fx chain too, and it means something
//! different: a patch's chain is the **instrument's** — a gunshot's corridor —
//! and it is applied to every note of that instrument independently, on its way
//! into the mix. That is the right home for the sound of an *object* and the
//! wrong home for the sound of a *room*, because a room is shared. If a piece
//! can only say "reverb" per patch, then every patch in it has to spell out the
//! same settings and keep them in sync by hand; nothing in the format suggests
//! that, so nobody does it, and the result is a set of sounds sitting next to
//! each other rather than a mix.
//!
//! So there are three places a chain can live, and the order between them is
//! the whole design:
//!
//! ```text
//!   notes ─► patch fx ─► track bus ─► track fx ─► × gain × pan ─┐
//!                                                               ├─► song fx ─► limiter
//!   notes ─► patch fx ──────────────────────────► × gain × pan ─┘
//! ```
//!
//! A **track** chain runs on that instrument's whole part, before its gain
//! reaches the master: it shapes an instrument. A **song** chain runs on the
//! sum: it shapes the piece. Any other ordering would blur the two into one
//! control whose meaning depended on how many tracks happened to be playing.
//!
//! Three consequences worth stating, because each is a decision rather than an
//! accident:
//!
//! - **Song fx run before the master limiter, never after.** The limiter is the
//!   guarantee that a bake cannot clip — [`super::render`] explains why it is
//!   not optional — and an effect able to add gain after it would withdraw that
//!   promise quietly.
//! - **A bus rings out.** A chain on the sum decays *once*, across everything,
//!   and past the last note, so each buffer grows by what its chain needs the
//!   same way a note's own fx tail grows the note. [`super::shape`] still cuts
//!   that to `tail` and `fit` afterwards; nothing here decides length.
//! - **A track with no chain is not bussed at all.** Its notes are added
//!   straight into the master at its gain, which is both the cheap path and the
//!   *bit-identical* one. Summing into a bus at unity and scaling once at
//!   fold-down is algebraically the same, but `(a + b)·g` and `a·g + b·g` are
//!   not the same `f32`, and a field whose whole promise is that leaving it out
//!   changes nothing cannot afford "nothing, to within rounding". `pan` keeps
//!   that promise the same way: dead centre is a literal `1.0` per side rather
//!   than the `0.99999994` the pan law computes there.
//!
//! ## How much of a part reaches the room
//!
//! A track's **`send`** says how much of it the song chain's `reverb` and
//! `delay` hear: the kick and the bass dry, the pad and the lead wet, all in
//! one room. #136 decided there would be no sends, because each effect's `mix`
//! was already the wet/dry control — and on a *track's* chain it is. On the
//! song's it is one number for the whole sum, and the two ways round it both
//! fail: a reverb per track is a room per track, which is what putting the
//! room on the song was for, and turning the song's reverb down takes the
//! space from the parts that wanted it. So this is that decision reversed, for
//! this one case and deliberately (#509), and no further: one send per track,
//! into the one chain the song already has. No buses, no returns, no routing.
//!
//! The mixer builds a second sum beside the master — every heard track at its
//! fader, its pan **and** its send — and [`fx::apply_chain_sent`] runs the
//! song's chain over the two, the rooms reading their tail from the second.
//! A send is applied where the fader is, after the track's own chain, for the
//! reason `pan` is: the chain is the instrument and the send is where it sits.
//!
//! **A send can move** ([`super::automate::Param::Send`]): the track is then
//! bussed like one whose fader moves, and its part is added into the second
//! sum sample by sample at the curve's value, after its fader.
//!
//! **The second sum is only built when it changes something**: a song whose
//! tracks all send everything and move no send, or whose chain has no room in
//! it, takes the
//! plain path and renders the samples it always did — which is the promise
//! that let the field arrive with no [`crate::SYNTH_VERSION`] bump.
//!
//! ## Where a part is put
//!
//! **`pan` is applied at the same moment `gain` is** — on the way into the
//! master, and never before. Both are answers to "where does this instrument
//! sit", one in level and one in position, and a track's own fx chain is the
//! instrument rather than its place in the mix: a delay that ran after the pan
//! would echo from wherever the fader had put the part, which is a delay that
//! changes character when an instrument is moved.
//!
//! The two multiply into one gain per side, so a note is walked once per
//! channel and not twice. [`crate::stereo::pan_gains`] carries the law.
//!
//! ## Keying one track from another
//!
//! A [`Fx::Compress`] on a track chain may name another track to listen to, and
//! this is the moment that can be honoured: the sum is the one place every
//! track exists separately, so it is the only place one part can be handed to
//! an effect running on a different one.
//!
//! **The part a key hands over is the instrument as played** — at unity, before
//! its own chain, its `gain` and its `pan`. Three things follow, and each is
//! the reason for the last:
//!
//! - **Turning the key track down does not stop it ducking.** A threshold is
//!   written against the part, so a duck that came unstuck the moment somebody
//!   moved a fader would be a setting nobody could keep. A kick that is felt
//!   rather than heard still presses the bass out of the way.
//! - **There is no ordering between tracks, so there is no cycle to refuse.**
//!   Two tracks keying each other is perfectly well defined here, because
//!   neither is waiting on the other's *output* — the pathological case the
//!   issue asked about simply cannot arise from this choice. Naming a track
//!   that does not exist is still refused, and so is a track keying itself,
//!   which is an ordinary compressor written the long way round.
//! - **It costs a buffer, and only where one was asked for.** A key is a third
//!   parallel copy alongside the bus and the measurement tap, kept for the
//!   tracks something actually names and for no others.
//!
//! ## Measuring on the way past
//!
//! The sum is also the one moment at which every track exists separately, so it
//! is where each one is measured. A report that says a mix is muddy without
//! saying which layer is muddying it sends its reader to change four
//! instruments at once and hope; [`crate::level::layer`] is what the rows are
//! for.
//!
//! It costs a buffer per track and a second add per note, and that is the
//! honest price rather than a free ride: a track with no chain is **not**
//! routed through a bus to get it, for exactly the rounding reason above. The
//! measurement takes a parallel copy instead of taking over the path, so the
//! samples handed out are bit-identical to the ones this mixer produced before
//! it measured anything. A song of one track skips it entirely — one row under
//! a one-line summary is the same sentence twice, so there is nothing to pay
//! for.

use super::Song;
use super::automate::{self, Param, Riding};
use super::clock::Clock;
use super::excerpt::Scope;
use crate::core::{RATE, SAMPLE_RATE};
use crate::fx;
use crate::level::{Cut, Layer};
use crate::patch::{Fx, sidechains};
use crate::stereo::{self, Stereo};

/// The buses a song is summed through: one master, and a private buffer for
/// each track that asked for a chain of its own.
///
/// It borrows the song rather than copying the gains and chains out of it, so
/// there is no second copy of the mix's settings to fall out of step with the
/// document — and so a caller cannot accidentally fold one song's buses down
/// against another song's tracks.
pub(super) struct Mix<'a> {
    song: &'a Song,
    master: Stereo,
    /// One slot per track, in track order: that track's own part of the mix.
    /// `None` until its first note arrives.
    ///
    /// It is the **bus** for a track that asked for a chain, and a
    /// measurement-only tap for one that did not — either way it holds what
    /// that instrument contributes and nothing else, which is what both jobs
    /// want.
    parts: Vec<Option<Stereo>>,
    /// One slot per track, holding that track's part **as played** for the
    /// tracks some sidechain names, and `None` for every other.
    ///
    /// Separate from [`Mix::parts`] rather than shared with it because the two
    /// hold different signals: a bus is consumed by its own chain and a
    /// chainless track's tap is stored at its fader, and a key is neither —
    /// see the module doc.
    keys: Vec<Option<Stereo>>,
    /// Whether a tap is kept for the tracks that have no chain of their own.
    ///
    /// False for a mix of fewer than two heard tracks, whose rows are not
    /// reported at all — see the module doc.
    measured: bool,
    /// How much of the song is being rendered: which tracks are heard, and
    /// which stretch of the result is kept.
    ///
    /// A whole render passes one that answers *everything*, so there is one
    /// path through this module rather than two.
    scope: &'a Scope,
    /// One slot per track: which of its parameters move across the piece.
    ///
    /// A track whose fader moves is bussed whether or not it asked for a
    /// chain, because a moving gain or pan cannot be one number applied on the
    /// way past — it has to be written onto that track's own part, and the
    /// part is what a bus is.
    riding: Vec<Riding<'a>>,
    /// The clock this render is actually running at. The only place a curve's
    /// beats become samples.
    clock: &'a Clock,
    /// What the song chain's rooms hear: every heard track at its placement
    /// times its send. `None` when that would be the master itself — see the
    /// module doc.
    sent: Option<Stereo>,
}

impl<'a> Mix<'a> {
    /// A master as long as the arrangement, and room for a part per track.
    ///
    /// Starting at the arrangement's own length rather than growing from empty
    /// is what keeps a song that ends on a rest that long; see
    /// [`super::render`].
    pub(super) fn new(
        song: &'a Song,
        arrangement_end: usize,
        clock: &'a Clock,
        scope: &'a Scope,
    ) -> Self {
        Self {
            song,
            master: Stereo::silence(arrangement_end),
            parts: song.tracks.iter().map(|_| None).collect(),
            keys: keyed(song),
            measured: scope.heard_count() > 1,
            riding: automate::riding(song),
            clock,
            sent: sends(song, scope).then(|| Stereo::silence(arrangement_end)),
            scope,
        }
    }

    /// Whether a note of this track is worth rendering at all.
    ///
    /// A track left out of a solo usually is not — that is the saving. The
    /// exception is a track something else is **keyed** from: it is played and
    /// not heard, so the duck a solo shows is the duck the mix has. Refusing
    /// the excerpt instead would be refusing the one question a solo of a
    /// ducked bass is usually asked to answer.
    pub(super) fn needs(&self, track: usize) -> bool {
        self.scope.heard(track) || self.keys[track].is_some()
    }

    /// Adds one rendered note of `track`, starting at sample `at`.
    ///
    /// Either straight into the master at the track's gain and pan, or into
    /// that track's bus at unity and dead centre, so its chain sees the
    /// instrument's whole part before either is applied.
    pub(super) fn add(&mut self, track: usize, src: &Stereo, at: usize) {
        // A track something is keyed from keeps its own copy, at unity and
        // ahead of everything else that happens to a part — which is what
        // makes a duck a property of the playing rather than of the mix.
        if let Some(key) = self.keys[track].as_mut() {
            mix_into(key, src, at, UNITY);
        }
        // Ahead of everything but the key, which is the whole of what a track
        // left out of a solo still does.
        if !self.scope.heard(track) {
            return;
        }
        let played = &self.song.tracks[track];
        if !played.fx.is_empty() || self.riding[track].needs_a_bus() {
            mix_into(self.parts[track].get_or_insert_default(), src, at, UNITY);
            return;
        }
        let placed = placement(played.gain, played.pan);
        mix_into(&mut self.master, src, at, placed);
        if let Some(sent) = self.sent.as_mut() {
            mix_into(sent, src, at, sending(placed, played.send));
        }
        if self.measured {
            // At gain and pan, like the master addition beside it and unlike a
            // bus: there is no chain here to see the part before the fader,
            // and the row this ends up in is what the track contributes.
            mix_into(self.parts[track].get_or_insert_default(), src, at, placed);
        }
    }

    /// Folds every bus down and applies the song's own chain, handing back the
    /// summed mix **unlimited** — limiting is the renderer's last word, not the
    /// mixer's — and one measured row per track, each cut at `cuts` the way
    /// the sum's own section rows will be.
    pub(super) fn finish(mut self, cuts: &[Cut]) -> (Stereo, Vec<Layer>) {
        let song = self.song;
        let mut parts = std::mem::take(&mut self.parts);
        // Taken out whole before any chain runs, so every track's chain sees
        // the same keys: what a key holds does not depend on how far down the
        // fold-down has got, which is what makes mutual keying meaningful.
        let keys = Keyed {
            song,
            parts: std::mem::take(&mut self.keys),
        };
        for (index, (track, part)) in song.tracks.iter().zip(&mut parts).enumerate() {
            let riding = self.riding[index];
            let rides = riding.moves_the_fader();
            let bussed = !track.fx.is_empty() || riding.needs_a_bus();
            let Some(bus) = part.as_mut().filter(|_| bussed) else {
                continue;
            };
            ring_out(bus, &track.fx, self.clock);
            fx::apply_chain_keyed(bus, &track.fx, RATE, &keys, self.clock);
            fx::close_tail(bus, &track.fx, RATE, self.clock);
            // The fader is written onto the part itself when it moves, and the
            // part then arrives at unity, because there is no single number to
            // add it at. After the chain, for the reason a static fader is:
            // the chain is the instrument, and the fader is where it sits.
            let placed = if rides {
                automate::ride(bus, track, riding, self.clock);
                UNITY
            } else {
                placement(track.gain, track.pan)
            };
            mix_into(&mut self.master, bus, 0, placed);
            if let Some(sent) = self.sent.as_mut() {
                match riding.send {
                    Some(curve) => {
                        automate::send_into(sent, bus, placed, curve, track.send, self.clock);
                    }
                    None => mix_into(sent, bus, 0, sending(placed, track.send)),
                }
            }
            if !rides {
                // Scaled after the fold-down rather than before it, so the
                // master is summed from exactly the samples it always was and
                // only the copy being measured moves.
                scale(bus, placed);
            }
        }
        ring_out(&mut self.master, &song.fx, self.clock);
        match self.sent.as_mut() {
            Some(sent) => {
                sent.grow_to(self.master.frames());
                fx::apply_chain_sent(&mut self.master, sent, &song.fx, RATE, self.clock);
            }
            None => fx::apply_chain_timed(&mut self.master, &song.fx, RATE, self.clock),
        }
        fx::close_tail(&mut self.master, &song.fx, RATE, self.clock);
        let layers = if self.measured {
            measure(song, parts, self.master.frames(), self.scope, cuts)
        } else {
            Vec::new()
        };
        (self.master, layers)
    }
}

/// Whether this render needs a sent mix beside the master: the song's chain
/// has a room in it, and some track that is heard sends less than all of
/// itself into it, or moves its send — a curve that opens to `1.0` is still
/// less than all of it until it gets there. Anything else is the master, and
/// is not worth a buffer.
fn sends(song: &Song, scope: &Scope) -> bool {
    let held_back = song.tracks.iter().enumerate().any(|(index, track)| {
        let moving = song
            .automation
            .iter()
            .any(|curve| curve.param == Param::Send && curve.track == track.name);
        scope.heard(index) && (track.send < 1.0 || moving)
    });
    has_a_room(&song.fx) && held_back
}

/// Whether a chain has a stage a send reaches: a `reverb` or a `delay`.
pub(super) fn has_a_room(chain: &[Fx]) -> bool {
    chain
        .iter()
        .any(|fx| matches!(fx, Fx::Reverb { .. } | Fx::Delay { .. }))
}

/// A placement scaled by how much of the part is sent, clamped to `0..=1` —
/// there is no sending more than all of a part, nor less than none of it.
fn sending((left, right): (f32, f32), send: f32) -> (f32, f32) {
    let send = send.clamp(0.0, 1.0);
    (left * send, right * send)
}

/// A slot per track, empty and ready to collect a part for the tracks some
/// sidechain names, and `None` for the rest.
///
/// Seeded up front rather than on demand because [`Mix::add`] is the note loop:
/// it has to know whether to keep a copy without asking the document about it
/// once per note.
fn keyed(song: &Song) -> Vec<Option<Stereo>> {
    let mut keys: Vec<Option<Stereo>> = song.tracks.iter().map(|_| None).collect();
    let named = song.tracks.iter().flat_map(|track| sidechains(&track.fx));
    for name in named {
        if let Some(index) = song.tracks.iter().position(|track| track.name == name) {
            keys[index] = Some(Stereo::default());
        }
    }
    keys
}

/// The parts a sidechained compressor on a track chain is allowed to listen to.
///
/// A name rather than an index, because a document names tracks and nothing in
/// it should have to know their order.
struct Keyed<'a> {
    song: &'a Song,
    parts: Vec<Option<Stereo>>,
}

impl fx::Keys for Keyed<'_> {
    fn part(&self, track: &str) -> Option<&Stereo> {
        let index = self.song.tracks.iter().position(|it| it.name == track)?;
        self.parts.get(index)?.as_ref()
    }
}

/// A placement that changes nothing: unity on both sides. What a track's own
/// bus is summed at, because its fader and its pan are applied later, on the
/// way to the master.
const UNITY: (f32, f32) = (1.0, 1.0);

/// The gain each side of a track's part is added at: its fader and its
/// position, multiplied into one number per channel.
///
/// One multiply rather than two passes, and the reason [`UNITY`] is a value
/// rather than a special case — everything that adds into a buffer here adds
/// at *some* placement, and a bus's happens to be the identity.
fn placement(gain: f32, pan: f32) -> (f32, f32) {
    let (l, r) = stereo::pan_gains(pan);
    (gain * l, gain * r)
}

/// One row per track, each measured over the length of the finished mix.
///
/// Padded to that length rather than measured over its own, because the rows
/// are read *against each other*: a hat that plays for four bars of a
/// forty-second piece does not take up more room than the pad under it, and a
/// mean over each track's own extent would say it did.
///
/// A track that never played still gets a row, saying it is silent. A missing
/// row reads as an oversight, and "the arp is not in this mix" is a finding.
///
/// A row is measured on **both channels**, interleaved, exactly as the mix
/// above it is. That is what keeps a hard-panned part from reading quiet: its
/// energy is all on one side, and a measurement of one side, or of a fold-down,
/// would report a number that says the instrument is missing rather than that
/// it is over there.
fn measure(
    song: &Song,
    parts: Vec<Option<Stereo>>,
    frames: usize,
    scope: &Scope,
    cuts: &[Cut],
) -> Vec<Layer> {
    let (from, to) = scope.keep(frames);
    song.tracks
        .iter()
        .zip(parts)
        .enumerate()
        // A track a solo left out has no row: it is not in this mix, and a
        // silent row would read as an instrument that failed to play rather
        // than one nobody asked for.
        .filter(|(index, _)| scope.heard(*index))
        .map(|(_, (track, part))| {
            let mut part = part.unwrap_or_default();
            if !part.is_empty() {
                part.grow_to(frames);
                // Over the same stretch the summary above it covers, for the
                // reason the padding beside it exists: these rows are read
                // against each other and against the whole, so all three have
                // to be measurements of the same piece of time.
                part.cut(from, to);
            }
            Layer::of(
                track.name.clone(),
                &part.interleaved(),
                stereo::CHANNELS,
                SAMPLE_RATE,
                cuts.to_vec(),
            )
        })
        .collect()
}

/// Grows `buf` by however long `chain` needs to decay into, at the tempo
/// `clock` plays.
///
/// A dry chain, and no chain at all, ask for nothing — so this is a no-op on
/// the path every existing song takes.
fn ring_out(buf: &mut Stereo, chain: &[Fx], clock: &Clock) {
    let tail = (fx::tail_seconds(chain, clock) * RATE).round() as usize;
    buf.resize(buf.frames() + tail);
}

/// Scales every sample of `buf` by a per-side gain.
fn scale(buf: &mut Stereo, (left, right): (f32, f32)) {
    for (slot, gain) in [(&mut buf.l, left), (&mut buf.r, right)] {
        for sample in slot.iter_mut() {
            *sample *= gain;
        }
    }
}

/// Adds `src` into `dst` starting at sample-frame `at`, at a per-side gain,
/// growing `dst` to fit.
///
/// The growth is what lets a song ring out: the last note's release tail
/// extends the buffer past the final beat instead of being truncated to it.
fn mix_into(dst: &mut Stereo, src: &Stereo, at: usize, (left, right): (f32, f32)) {
    let end = at + src.frames();
    dst.grow_to(end);
    for (slot, (source, gain)) in [(&mut dst.l, (&src.l, left)), (&mut dst.r, (&src.r, right))] {
        for (into, sample) in slot[at..end].iter_mut().zip(source) {
            *into += sample * gain;
        }
    }
}

/// The arithmetic, by the number rather than by the piece of music.
///
/// What defends this module otherwise is a suite that renders whole songs and
/// asks how long they are and how loud they came out — questions a mixer that
/// **subtracted** would answer correctly, because a peak is a magnitude and a
/// length does not care about a sign. That is #60, the bug this crate's
/// mutation surface was widened to catch: `Mix::add` mixing by subtraction with
/// the whole suite green.
///
/// So these say where a part lands, which way up, and at what gain on each
/// side — the three things a sum has to get right and that nothing measuring a
/// finished mix can see.
#[cfg(test)]
mod tests {
    use super::*;

    /// A signal that is `value` on the left and its negative on the right, so
    /// a test can tell the two channels apart and a sign flip cannot hide in a
    /// symmetry.
    fn lopsided(frames: usize, value: f32) -> Stereo {
        Stereo {
            l: vec![value; frames],
            r: vec![-value; frames],
        }
    }

    /// Whether a second sum is built changes the cost of a render and, in a
    /// room, almost nothing audible — so it is asked here, by the number: a
    /// room and a track holding something back, where a moving send counts as
    /// holding back and a moving fader does not.
    #[test]
    fn a_sent_sum_is_built_for_a_partial_or_moving_send_into_a_room() {
        let song = |send: f32, room: bool, param: &str| -> Song {
            let fx = if room {
                r#"{ "fx": "reverb", "size": 0.5, "damp": 0.5, "mix": 0.2 }"#
            } else {
                ""
            };
            serde_json::from_str(&format!(
                r#"{{ "bpm": 120, "tracks": [{{ "name": "pad", "patch": "p", "send": {send} }}],
                     "patterns": {{}}, "arrangement": [], "fx": [{fx}],
                     "automation": [{{ "track": "pad", "param": "{param}",
                                      "points": [{{ "beat": 0, "value": 0.5 }}] }}] }}"#
            ))
            .expect("the fixture reads")
        };
        let whole = |song: &Song| {
            let scope = Scope::of(song, &Default::default(), &Clock::at(120.0)).expect("whole");
            sends(song, &scope)
        };
        assert!(
            !whole(&song(1.0, true, "gain")),
            "everything sent, a fader moving"
        );
        assert!(whole(&song(1.0, true, "send")), "a moving send");
        assert!(whole(&song(0.5, true, "gain")), "a written partial send");
        assert!(!whole(&song(0.5, false, "send")), "no room to send into");
    }

    /// The sum is a sum: the source arrives on top of what was there, the same
    /// way up, at the frame it was given.
    #[test]
    fn a_part_is_added_where_it_starts_and_the_way_up_it_came() {
        let mut master = Stereo::centred(vec![0.25; 6]);
        mix_into(&mut master, &lopsided(2, 1.0), 3, UNITY);
        assert_eq!(master.l, vec![0.25, 0.25, 0.25, 1.25, 1.25, 0.25]);
        assert_eq!(master.r, vec![0.25, 0.25, 0.25, -0.75, -0.75, 0.25]);
    }

    /// The destination grows to fit rather than truncating — this is what lets
    /// a song ring out past its final beat.
    #[test]
    fn a_part_landing_past_the_end_extends_the_mix() {
        let mut master = Stereo::silence(2);
        mix_into(&mut master, &lopsided(3, 0.5), 4, UNITY);
        assert_eq!(master.frames(), 7, "four frames in, three long");
        assert_eq!(master.l, vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5]);
    }

    /// Each side is scaled by its own number on the way in, so a placement is
    /// applied once and in the same pass as the fader.
    #[test]
    fn each_side_arrives_at_its_own_gain() {
        let mut master = Stereo::silence(1);
        mix_into(&mut master, &Stereo::centred(vec![1.0]), 0, (0.25, 0.75));
        assert_eq!(master.l, vec![0.25]);
        assert_eq!(master.r, vec![0.75]);
    }

    /// [`scale`] multiplies, and multiplies each side by its own gain — the
    /// pass that puts a bussed track's fader and pan onto the copy that gets
    /// measured.
    #[test]
    fn scaling_a_bus_multiplies_each_side_by_its_own_gain() {
        let mut bus = lopsided(2, 1.0);
        scale(&mut bus, (0.5, 0.25));
        assert_eq!(bus.l, vec![0.5, 0.5]);
        assert_eq!(bus.r, vec![-0.25, -0.25]);
    }

    /// A fader and a position multiply into one number per side, and a centred
    /// track's is exactly its fader — the promise `pan` has to keep.
    #[test]
    fn a_placement_is_the_fader_times_the_position() {
        assert_eq!(placement(0.8, 0.0), (0.8, 0.8));
        let (l, r) = placement(0.5, -1.0);
        assert!(
            (l - 0.5 * std::f32::consts::SQRT_2).abs() < 1e-6,
            "left {l}"
        );
        assert_eq!(r, 0.0);
        assert_eq!(
            UNITY,
            placement(1.0, 0.0),
            "a bus is summed at the identity"
        );
    }
}
