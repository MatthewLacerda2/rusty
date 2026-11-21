# `SYNTH_VERSION`: what each bump was, and what was checked

The rule lives on the constant itself, in `src/lib.rs`, with a table of the
versions; `CLAUDE.md` beside this file says when a change needs one. This page
is the record behind that table: what each branch that faced the question
decided, and how it checked. Read it before deciding a borderline case — the
answer is usually here already.

**What "differently" has meant so far**, because the first change to face
this question was an ambiguous one and the answer is easier to reuse than
to re-derive: *any* representable recipe, not *most* recipes and not an
audible difference. Version 2 added curved envelope segments and a pitch
envelope, both no-ops at their defaults — 23 of 25 probe recipes came back
byte-identical, every envelope corner and every sane vibrato among them.
The two that moved were pitch LFOs deeper than the ten octaves the
frequency track now bounds, which render nothing but aliasing either way.
It was bumped regardless. A spurious bump costs one re-render of audio that
turns out identical; a missed one leaves a project serving samples its
recipe no longer describes, which is the failure this constant exists to
prevent — so where the two are in tension, bump.

**What has not moved it**, since the easy half of the rule is worth writing
down beside the hard half: a **new** effect, source or stage that an
existing recipe cannot be using. The EQ arrived as a new `Fx` variant and a
module nothing calls without one, and no field of any existing variant
changed — 25 probe recipes covering every source, every envelope corner,
every LFO target and every existing effect came back byte-identical across
the change. A recipe that does not name the new thing renders what it
always did, by construction rather than by luck, and that is the case where
leaving this number alone is right.

The **compressor** is the second one of those, and it went further into the
mixer than the EQ did — a new buffer per keyed track, and a second entry
point into the fx chain — so it was checked rather than argued: 24 probes
across all four sources, every envelope corner, all three LFO targets,
every existing effect, all three chain locations, chords, step strings,
keys and degrees, swing and humanise, the arrangement transforms and the
fit, fade and tail fields came back byte-identical. The extra buffer is
only kept for a track something names, and nothing names one in a recipe
written before this existed.

The **chorus** is the third, and the plainest of the three: a new `Fx`
variant, two modules nothing calls without one, and no field of any
existing variant touched. Checked anyway, against `84ad2a3` — 30 probes
covering all four sources, every envelope corner including curves and a
pitch envelope, all three LFO targets, a velocity-routed filter, every
effect that existed then wet and dry, a chain of four, and eight songs
spanning swing, humanise, a track bus, a song chain, a fade, a `fit`, an
arrangement transform, a key with degrees and a step string. All 30 came
back byte-identical.

**A new *source* is the same case as a new effect**, and it was checked the
same way rather than reasoned from that sentence. `additive` added a
[`Source`](patch::Source) variant, a `Partial` type, a renderer module and
four refusals reachable only from the new arm; 46 probes — every source,
all three envelope curves, all three LFO targets, both filter kinds, a
pitch envelope, every effect and a two-stage chain, each at two velocities,
plus six songs covering swing, all three humanise axes, a pan, a track bus,
a song chain, a fade, an arrangement transform, a `fit` and a key, as
`360ac75` had them — came back byte-identical. The measure is the one above and not
the size of the diff: *can a recipe written before this change name the new
thing?*

**New notation is the same case**, and worth naming separately because it
does not look like it. A song key and scale degrees added a `Song` field
and a fourth kind of pattern entry, reordered the enum holding the other
three, and replaced the note loop's pitch step — and 18 probes across the
arrangement transforms, swing, all three humanise axes, the fit modes, the
fades, both kinds of chain, all four sources, chords and step strings still
came back byte-identical. The measure is the same one either way: *can a
recipe written before this change name the new thing?* Not *did the code
around it move*.

**`fm4` is the second new source, and it needed a wider probe than the
first**, which is the part worth writing down rather than the verdict. The
verdict is the one above: a recipe written before it cannot name `fm4`, so
the number does not move.

What made the check different is that this one **touched code an existing
recipe does reach**. The source stage takes a new argument — the gate, for
the per-operator envelopes — so every source's call site moved; `core/fm.rs`
became a directory; and `additive`'s own Nyquist helper was folded into a
shared `core::nyquist` module, which means the *previous*
source was edited by this change. So the probes had two jobs, not one: show
that nothing can name `fm4`, and show that `additive` still renders exactly
what it rendered yesterday.

34 probes against `9e73ead`, all byte-identical. Twenty-one cover the
sources that predate `additive`, both FM ratio kinds, curved envelopes
either way, a pitch envelope, all three LFO targets, both filter kinds with
a velocity routing and a negative envelope amount, every effect, and a
chain of four. Four are `additive` alone, deliberately placed to work the
helper that moved: a sixteen-partial series played low enough to carry all
of it, high enough to lose most of it, and at the top of the keyboard where
almost nothing survives. Nine are songs — chords, step strings, a key with
degrees and a diatonic lift, swing and all three humanise axes, a stretch
`fit` with fades and an exact tail, a sidechained track chain beside a song
chain, a patch named by reference, and two additive songs, one of them
under a vibrato so the ceiling is decided by a moving pitch track rather
than a flat one.

Editing a source is normally exactly the case that bumps this number, and
the reason it does not here is worth being explicit about rather than
leaving to a reader to infer: the rule is *the samples move*, and the four
`additive` probes exist precisely because that claim about the helper's
move is the one nobody should take on trust. This is the compressor's case
again — further into shipping code than the change before it, and so
measured instead of reasoned about.

**Noise colour and oscillator unison are a leave-it-alone case of a new
shape**: the first where the new thing is a *field on an existing variant*
rather than a variant of its own, which is the shape that usually does move
this number — so it was checked rather than argued. `Source::Noise` grew a
`color` and `Osc` grew `voices` and `spread`, each defaulting to what the
crate already did and each skipped on the way out. 37 probes against
`eecdfcf` — all six sources, a stack of four, an `fm4` routing with
per-operator envelopes and a velocity index, an additive series both
harmonic and stretched, every envelope corner including both directions of
curve, a pitch envelope, the filter swept and velocity-routed, all three
LFO targets, every effect, a chain of four, and nine songs covering step
strings, chords, a key with degrees, swing, humanise, a track bus, a song
chain, a sidechain, panning, an automated fader and a drifting pan, `fit`,
`fade`, `tail` and an arrangement transform — came back byte-identical. A
recipe that names neither field renders what it always did, and the
defaults are what make that true by construction.

**Per-note articulation is that case again**, and it came out the same way
for the same reason: a mark is an optional field on each of the four ways an
entry is written, and a document written before it existed names none of
them, so every note in it is played exactly as written.

Checked against `0b4fd58` rather than argued from that sentence — 30 probes,
all byte-identical, encoded bytes and length both. All six sources,
including `fm4` with per-operator envelopes and a velocity index and both
coloured noises; a unison stack; every envelope corner and both directions
of curve; a pitch envelope; all three LFO targets; both filter kinds with a
velocity routing; every effect wet and dry, and a chain of several.
Twenty-one of them are songs, covering chords, step strings, keys and
degrees, swing, all three humanise axes, the arrangement transforms and the
track filter, a pan, a track bus, a song chain, a sidechain, an automated
cutoff and an automated fader, a fit, a fade and a tail.

The measure is the one this section keeps returning to, and it is worth
saying plainly for a notation: the note loop was edited — every note now
reads a mark before its gate, its velocity and its onset are decided — and
that is exactly why the probes were run rather than the sentence trusted. A
document that names no mark takes the identity through all three.

**Version 3 is the far end of that scale** and needs no argument at all: the
crate went stereo, so every bake in every project is a different file, of a
different channel count, and there is no recipe anywhere that renders to
what it used to.

**Version 4 is the master limiter changing what it limits against** — the
true peak rather than the loudest sample, under a ceiling of −1 dBTP rather
than 0.98. Every bake passes through it, so this is the *any representable
recipe* case at its widest: a recipe that used to touch the limiter comes
back about a decibel down, and one that never did comes back reconstructed
against a lower number and touches it now. There is no defaults argument to
have here and no probe worth running.

**Version 5 is `fm2`'s two operators starting where the note's seed says**
rather than both at zero — the last source that was still phase-locked, and
the leftover from the change that unlocked the oscillator stack. Every note
of every `fm2` patch is different samples, so this is the ordinary case of
the rule rather than an argument about defaults: there is no field to leave
unwritten and no recipe that opts out, because the phases were never in the
document.

No probe corpus, and the diff is why. The only edit outside `core/fm/two.rs`
is the call site handing it the seed it already had in hand — no shared
helper moved, no stage was reordered, and nothing any other source reads was
touched — so a recipe that does not name `fm2` cannot have moved, and one
that does moved by construction. Baking a corpus would be confirming a
sentence the diff already settles, which is the case this section names as
waste.

**Version 6 is a filter that modulates in octaves**, and it is the *any
representable recipe* case again: a cutoff is now
`cutoff × 2^(env_octaves × env + vel_octaves × vel + lfo)` where it used to
be `cutoff + env_amount × env + vel_cutoff × vel`, then multiplied by the
LFO. Every filtered patch that modulates at all comes back different, and
the ones that do not — no envelope depth, no velocity routing, no cutoff
LFO — come back identical, because a zero exponent is a multiply by one
exactly as a zero offset was an add of nothing.

That last sentence is the half worth checking rather than asserting, since
it is a change to the arithmetic and not a new optional field. Checked
against `b2f3fbf8`, the commit before it — **33 probes, all
byte-identical**, encoded bytes and length both. Every source, including
`fm4` with per-operator envelopes, an additive series, a unison stack and
all three noise colours; a static lowpass and a static highpass, which read
a cutoff and modulate nothing; a pitch LFO and an amp LFO; a pitch
envelope; both signs of envelope curve; every effect on its own and a chain
of four; five songs covering swing with all three humanise axes, a song
chain, an automated fader and an automated pan; and five of the worked
examples out of `docs/recipes.md`.

The other half needs no probe: the old build has no field in which to write
the new depth, so a recipe that modulates its filter is a different
document on either side of this line rather than the same one rendered
twice.

**The same version carries the filter's two new modes and its slope**, and
neither would have earned a bump alone. [`FilterKind::Bandpass`](patch::FilterKind::Bandpass) and
[`FilterKind::Notch`](patch::FilterKind::Notch) are new words, so no document that exists can be
reinterpreted by them; [`Filter::slope`](patch::Filter::slope) is a new optional field defaulting
to the single pole pair every patch on disk already had, which is the case
this section says a diff answers without rendering anything. It was
re-checked anyway, because the pole pair is now reached through a cascade
rather than called directly and that is a stage reordered rather than a
field added: the same 33 probes, run once more over the whole branch, all
byte-identical again.

**Version 7 is `fm4`'s four operators starting where the note's seed
says**, and it closes the set: no source in this crate is phase-locked any
more. It is version 5's change one source further along, and the ordinary
case of the rule rather than an argument about defaults — the phases were
never in the document, so there is no field to leave unwritten and no
recipe that opts out. Every note of every `fm4` patch is different samples,
and a `fm4` patch is the one this matters most at: four operators stand in
six relationships where `fm2`'s two stand in one, and in FM a relationship
between operators is the timbre rather than a decoration on it.

**The same version carries a ping-pong `delay`**, which is the opposite
case and would have earned nothing on its own.
[`Fx::Delay`](patch::Fx::Delay)'s `ping_pong` is a new optional flag,
`false` by default and skipped when it is, so a recipe that never writes it
serialises to the bytes it always did and renders through the same
per-channel lines it always did — the case this section says a diff answers
without rendering anything. A recipe that *does* write it is a different
document on either side of this line rather than the same one rendered
twice, since the old build has no field to write it in.

The two are on one branch and share one bump on purpose. A bump misses
every cached bake in every project on disk, and doing that twice in a night
for two changes that could travel together is a cost paid for nothing.

Checked rather than reasoned about, since the `fm4` half is rendering
maths: **50 probes against `3f33e49e`**, the `main` this branch merges
into —
every source including all three noise colours, a unison stack, an additive
series, `karplus`, `fm2` and six `fm4` patches; all four filter kinds, both
slopes, an envelope-and-velocity cutoff and all three LFO targets; a pitch
envelope and both signs of amp curve; every effect on its own and a chain
of four; five songs covering swing, all three humanise axes, a song chain,
a pan and a track delay; and every worked example in `docs/recipes.md`.

**Eleven moved and thirty-nine did not**, which is exactly the split the
two halves predict. The eleven are the six `fm4` patches, the `fm4` worked
example, and the four songs whose bass track is an `fm4` — nothing else.
Every delay probe came back byte-identical, encoded bytes and length both,
including the song whose lead track carries one: the flag defaults to the
per-channel lines, and a recipe that does not write it is not a different
document.

**A song's tempo map left it alone**, and it is the notation case again
with the most shipping code moved under it so far. `tempo` is a new
optional `Song` field, skipped when empty, so no document on disk can name
it — but every beat a song renders now goes through a clock that *might*
carry one: note onsets, gate lengths (now measured from where a note is
played), the arrangement end, the fader's beat per sample, the `fit` plan
including the stretched tempo, and the windows and section rows. The claim
that a song without a map takes the `f32` arithmetic it always did, in the
same order, is exactly the kind nobody should take on trust, so it was
measured.

**24 probes against `3e2adb0c`, all byte-identical**, encoded bytes and
length both: the three worked songs in `docs/recipes.md`, eleven of its
`fields` examples spliced into the 96 bpm one (fit in all three modes, fade,
tail, swing, humanise, a key, the song chain, automation), and nine of a
three-track song — karplus lead with articulations, a filtered saw bass
playing a chord, a step-string hat, degrees in D minor and a diatonic lift
— plain, swung, humanised, with gain, pan and cutoff automation, looped
with a fader curve across passes, stretched with a cutoff curve and fades,
played once with an exact tail, with fades and an exact tail, and with a
song reverb and a track delay. The map's own arithmetic is new rendering
that no existing recipe can reach, and it is held to the analytic
beat-to-seconds integral in `song::clock::map`'s tests instead.

**Version 8 is a one-shot's file ending where its sound does** (#970).
`bake_note` used to write the whole buffer the renderer had sized for the
worst case — gate, release and every effect's longest tail — so a 0.1 s
pop through a small room baked to 1.2 s of file. It now drops everything
after the last frame within 60 dB of the peak (`core::trim`). Every patch
bake on disk is a different file under the same recipe, which is the case
this constant exists for, so it is bumped even though no sample that is
kept moves: the trim runs after the limiter and only shortens the buffer,
so the bytes before the cut are the old bytes by construction, and the
diff answers it without a probe corpus. Songs are untouched — they render
through `render_note` untrimmed and keep deciding their own length with
`tail` — but they share the number, so a song bake misses the cache once
and comes back byte-identical.

**rusty re-bakes** every one-shot the next time it moves its pin past this
version, and gets the shorter files.

**Unison `width` and a one-shot's `vary` left it alone** (#1124, #1038).
Both are new optional fields, skipped at a default that renders what was
always rendered — but `width` moved the oscillator stack's inner loop from
one buffer to two, scaling each voice by its pan gains, which is an edited
note loop and so measured rather than argued: **53 probes against
`c186994`, all byte-identical** — every kit preset at seeds 0 and 7, and
the landing page's song, whose leads and pads are unison stacks.

**Version 9 is three changes to what every score sounds like, made together
under one bump** (#1123):

- **A `cutoff` curve is read inside the note**, not once at its onset: one
  chord held through a build now opens with it.
- **The compressor and the limiter release like real ones**: recovery is a
  time constant on the gain in dB, the same length however deep the duck, and
  the level they react to is held over 20 ms, so a bass or a kick no longer
  makes the gain ripple at twice its own frequency. One shared module
  (`fx::gain`) replaced the two copies of the ramp passes.
- **Ring-outs are not cut while audible**: a reverb's and a delay's tail is
  derived from its feedback (the time to fall 60 dB), the cap on a chain's
  tail rose from 6 s to 15 s, and the last 10 ms of every ring-out is faded.

Every recipe that touches a compressor, a limiter (every one), a reverb, a
delay or a cutoff curve moves, which is every bake, so there was no
defaults argument to have. A probe corpus was baked before and after anyway,
because the maintainer asked to see what moved: the PR for #1123 has the
per-recipe table. **rusty re-bakes** everything the next time it moves its pin
past this version.

**A delay in beats, a track's own feel and a reverb's pre-delay left it
alone** (#1122). Five new optional fields — `Fx::Delay::beats`,
`Fx::Reverb::predelay`, and a track's `push`, `swing` and `humanize` — each
skipped at a default that renders what was always rendered. But a delay line
that can follow a tempo map is read at a computed offset rather than by a
stepping pointer, and every chain now carries the clock through, so the delay
loop was edited and measured rather than argued: **30 probes against
`44914ad`, all byte-identical** — a two-track song (an inline saw stack with
a patch delay and reverb, a noise step string with a patch ping-pong) under
three track chains (none, a delay, a ping-pong into a full-size room), each
with and without a track `send`, in five songs: plain, swung and humanised,
a song chain of delay and reverb, the same with a ping-pong under a
`stretch` fit, and a ramped tempo map under a long room.

**Layers left it alone** (#1771): `Patch::layers` is skipped when empty, and
the note loop that sums them renders the top-level path with the same
arithmetic in the same order, so **78 probes against `33625e9` — every kit
preset at three pitches, seeds and velocities, under `vary` — came back
byte-identical**.

**The LFO's shapes, onset, beats and second slot left it alone** (#1772):
`Lfo::shape`, `delay`, `fade` and `beats` and `Patch::lfo2` are each skipped
at a default that renders what was always rendered, but `lfo_wave` became a
per-note curve shared by every layer, so it was measured: **157 probes
against `3b424e7` — every kit preset at three pitches, two seeds and
velocities, and the landing page's song — came back byte-identical**.

**Version 10 is the filter's oversampling interpolating its half-steps**
(#1788). The SVF runs two half-steps per output sample; the first used to
take the same input as the second (a zero-order hold), which images every
tone `f` at `sr − f`. A highpass passes that image and keeping every other
output folds it back onto `f`, so a highpass's stopband floor was ~6 dB per
octave whatever its slope: 560 Hz under a 24 dB highpass at 9 kHz came out
at −24.7 dB where the slope gives −96. The half-step now takes the cubic
halfway point of four samples (`[-1, 9, 9, -1] / 16`), which puts it at −95.
A straight line between two samples reached only −59, and averaging the
two half-step outputs, which also clears it, took 3.7 dB off every open
lowpass at 15 kHz, so neither was taken.

Every recipe with a filter moves, by a quarter-sample shift at least (the
hold ran every filter's output early by about that much), so there was no
defaults argument to have. **186 probes against `ce563b0`** — every kit
preset at C3, C4 and C5, seeds 0 and 7, one second at velocity 0.9 —
measured by level and by energy share under 500 Hz, 500 Hz–4 kHz and above:
the six presets with no filter came back byte-identical; the lowpass-only
ones (bass, brass, epiano, pad, pluck, strings) within 0.01 dB and 0.1
points of every share. The highpasses over tones moved most, as the fix
means them to: ride, tambourine, open hat, crash and hat keep up to 12
points less of their energy between 500 Hz and 4 kHz, within 2 dB of
level. The bandpasses and highpasses over noise lost the false brightness
the image had added, 0.5 to 2.8 points moving from above 4 kHz into 500 Hz–4
kHz, within 0.4 dB; a riser's length, cut where its sound falls under the
floor, moved ±24 ms. The PR for #1788 has the per-preset table. **rusty
re-bakes** everything with a filter the next time it moves its pin past
this version.
