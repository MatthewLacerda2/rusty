## `Sound`

Agent-composed **procedural sound authoring** (#357): describe an instrument as a
*patch* and **bake** one note of it to a stereo `.wav`. That single note is the whole
one-shot SFX story — a gunshot, an impact, a footstep, a UI blip are each one
rendered note of a noise / Karplus / FM patch — and the returned path drops straight
into `Audio.PlayAt` or an `AudioSource`'s `clip`, so a sound the agent invented is
audible in the same script that made it.

A bake is free, and a recipe reproduces byte-for-byte. A sound whose realism *is*
the point (a door with a latch, glass, gravel, cloth, a casing on concrete, a voice
line) is beyond synthesis: bring it as an audio file, which plays like any other clip.

**The synthesiser is zimmer**, the engine's own crate `rusty-zimmer` (`crates/zimmer/`).
It does no I/O, so `Sound` is a thin adapter over it: rusty turns the Lua table into
zimmer's document, zimmer renders, rusty writes the file. A bake changes only when
zimmer's rendering changes, and `rusty_zimmer::SYNTH_VERSION` moves with every such change.
**This page carries the whole patch and song vocabulary**: every key a recipe can
write, with its default and range, is in a table below.

A patch is authored as a Lua **table** whose shape mirrors the on-disk JSON document
one-to-one, so the same document describes a Lua-built patch and one loaded from a
file. Every object in it refuses unknown keys, so a misspelled field is an error, not
a silent default. Bakes are **deterministic**: the same patch + note + `seed` always
writes a byte-identical WAV (every stochastic source draws from a seeded integer
hash, never wall-clock or unseeded RNG). Output is always **stereo**, 16-bit PCM,
44.1 kHz, and always passes a true-peak **limiter**, so a bake can never clip.

| Function | Signature | Returns |
|---|---|---|
| `Sound.Bake` | `(patch, note, path [, opts])` | the written `path`, then its [level](#how-loud-it-came-out-378) |
| `Sound.ToJson` | `(patch)` | the patch's canonical JSON string |

`patch` is a table **or** its serialized JSON string (from `Sound.ToJson`, or a saved
`.json`) — both forms decode alike, with the same errors. Its enum values (`kind`, `wave`, `fx`) are read in any case; a patch or song file read by path (a song's named patch, `Sound.Survey`) must hold the canonical spelling, which `Sound.ToJson` writes. `note` is either a **name** — a letter `A`–`G`, any accidentals (`#`/`s`
sharp, `b`/`f` flat), then the octave, e.g. `"C#4"`, `"Bb3"`, `"C-1"` — or a **MIDI
number** (`60` is middle C, `69` is A4 = 440 Hz; fractions are legal microtones).
Pitch is equal temperament: `f = 440 × 2^((midi − 69) / 12)`. A rejected patch, note
or option raises a Lua error naming what was wrong, and writes no file.

`opts` is optional, and so is every field in it; any other key is an error:

| Field | Default | Meaning |
|---|---|---|
| `duration` | `0.5` | Gate length in seconds — how long the note is *held*. The amp release rings out **after** it and an fx chain adds its own tail, and the file **ends where the sound does**: past the gate for a held note, short of it for a hit that decays to silence first (that hit writes the same file under any longer `duration`). |
| `velocity` | `0.8` | Striking force, `0..1`, scaling the note's peak amplitude. |
| `timbre` | `0.0` | How far this strike's *brightness* sits from its level, in velocity units: added to `velocity` only for the routings that read velocity as effort (a filter's `vel_octaves`, `fm2`'s `vel_index`), so a note can be brighter without being louder. |
| `glide` | none | `{ semitones, seconds }` — the note starts `semitones` away from its pitch (positive = above) and slides onto it over `seconds`. Both fields required. |
| `seed` | `0` | Seed for the stochastic sources (noise, the Karplus pluck, oscillator phases). |

### The patch shape

The signal path is **fixed** — `source → filter → amp envelope → fx`, with an
optional pitch envelope, up to three extra `layers` summed in before the fx, and up
to two LFOs. You choose what fills each stage, never how they connect; that is what
makes a patch playable as a note. `source` and `amp` are mandatory, the rest
optional.

```lua
{
  source = { kind = "osc_stack", oscs = {
    { wave = "saw",    detune_cents = -7, gain = 0.5, octave = 0 },
    { wave = "square", detune_cents =  7, gain = 0.5, octave = -1 },
  } },
  filter = { kind = "lowpass", cutoff = 1200, resonance = 0.4,
             env_octaves = 1.4, adsr = { a = 0.01, d = 0.2, s = 0.3, r = 0.2 } },
  amp    = { a = 0.005, d = 0.1, s = 0.7, r = 0.3 },
  lfo    = { rate = 5.0, depth = 0.5, target = "pitch" },   -- pitch | cutoff | amp
  fx     = { { fx = "delay",  time = 0.25, feedback = 0.35, mix = 0.3 },
             { fx = "reverb", size = 0.6,  damp = 0.5,      mix = 0.2 } },
}
```

Example — bake a gunshot and fire it where the shot happened:

```lua no-run
local clip = Sound.Bake({
  source = { kind = "noise" },
  amp    = { a = 0.0, d = 0.09, s = 0.0, r = 0.06 },
  filter = { kind = "lowpass", cutoff = 300, resonance = 0.5, env_octaves = 4.4,
             adsr = { a = 0.0, d = 0.05, s = 0.0, r = 0.05 } },
  fx     = { { fx = "reverb", size = 0.4, damp = 0.6, mix = 0.15 } },
}, "C2", "assets/sounds/gunshot.wav", { duration = 0.12, seed = 9 })

Audio.PlayAt(clip, muzzle.x, muzzle.y, muzzle.z, 0.9)
```

> **Faithfulness:** the read-site is the engine's own audio decoder — a baked `.wav`
> is decoded by the same `ClipCache` path an imported clip is. A re-bake to a path
> already played plays the new file from the next `Audio.Play*`, and a path played
> before it was baked plays once it is (#1208).

Every key a patch takes is in the tables below: what it is, its default when left out
(*required* when it has none), and its unit or range. A value past a stated range is
**clamped** where the table says so and **refused** (a Lua error naming it) where it
says that; nothing outside a table is a key.

#### The patch

| Key | Default | Meaning |
|---|---|---|
| `source` | *required* | What makes the raw tone: one [source](#sources), tagged by `kind`. |
| `amp` | *required* | The amplitude [envelope](#envelopes-adsr): what turns a continuous tone into a note. |
| `filter` | none | The [filter](#the-filter), with its own envelope. |
| `pitch_env` | none | A one-shot [sweep of the note's own pitch](#the-pitch-envelope): absent, the note holds the pitch it was played at. |
| `layers` | none | Up to **3** more sounds in the same note, each its own [layer](#layers), summed with this one before `fx`. |
| `lfo`, `lfo2` | none | Up to two [LFOs](#lfos); two aimed at one target add. |
| `fx` | none | The [effects](#effects) the whole note goes through, in list order. |

#### Sources

`kind` picks one; the other keys belong to that kind only.

| `kind` | Keys | What it is |
|---|---|---|
| `osc_stack` | `oscs` | Up to **4** [oscillators](#oscillators) summed, normalised by the sum of their gains: the classic subtractive head. An empty list, more than 4, or every `gain` at or below 0 is refused. |
| `karplus` | `damping`, `brightness` | A plucked string (Karplus-Strong): guitars, basses, harps, marimbas. |
| `noise` | `color` | Every frequency at once: the head of a gunshot, an impact, a footstep. |
| `fm2` | `ratio`, `index`, `vel_index`, `mod_decay` | Two-operator FM, one sine bending another: electric pianos, bells, metallic hits. |
| `fm4` | `algorithm`, `operators`, `vel_index` | Four-operator FM wired by an [algorithm](#four-operator-fm): brass, FM basses, pads, a bell over a body. |
| `additive` | `partials` | Up to **16** sine [partials](#partials), each placed, weighted and faded on its own: organs, bowed and blown sustains, glass. |

| Key | Default | Meaning |
|---|---|---|
| `damping` | `0.996` | *karplus* — feedback per round trip, `0..1` (clamped): how long the string rings. |
| `brightness` | `0.5` | *karplus* — the pluck, `0..1` (clamped): `0` a soft, pre-filtered pluck, `1` full noise (a hard pick). |
| `color` | `"white"` | *noise* — `white` (equal energy per hertz: a hiss, cymbals, air), `pink` (−3 dB an octave: wind, surf, rain, room tone, a snare's body) or `brown` (−6 dB an octave: thunder, rumble, the low half of an impact). |
| `ratio` | *required* | *fm2* — the modulator's frequency as a multiple of the note's. Whole numbers are tonal, fractions metallic. At or below `0` is refused. |
| `index` | *required* | *fm2* — modulation depth: higher adds sidebands, so brightness. |
| `vel_index` | `0` | *fm2*, *fm4* — depth a full-velocity strike adds (to `index`, or to every *modulator* operator's `level` in `fm4`), floored at 0: a bell tapped against a bell hit. |
| `mod_decay` | `0.3` | *fm2* — seconds for the modulator's own decay: how fast the bright attack collapses to a near-sine body. |
| `algorithm` | *required* | *fm4* — how the four operators are wired (below). |
| `operators` | *required* | *fm4* — exactly **4** [operators](#four-operator-fm), numbered 1–4 in the order the algorithms name them. |
| `oscs` | *required* | *osc_stack* — the oscillators. |
| `partials` | *required* | *additive* — the partials. Empty, more than 16, or every `gain` at or below 0 is refused. |

#### Oscillators

One entry of an `osc_stack`'s `oscs`.

| Key | Default | Meaning |
|---|---|---|
| `wave` | *required* | `sine` (one harmonic), `triangle` (odd harmonics falling away: soft), `saw` (every harmonic: the subtractive starting point) or `square` (odd harmonics: hollow, chiptune). `saw` and `square` are band-limited. |
| `detune_cents` | `0` | Detune from the played pitch, in cents (100 = a semitone). Small opposing detunes across a stack make a fat lead. |
| `gain` | `1` | Weight in the stack; the stack is normalised by the sum of its gains. |
| `octave` | `0` | Whole-octave transpose: `-1` is a sub-oscillator. |
| `voices` | `1` | Unison: how many detuned copies sound at once, `1..7` (refused outside). The copies are normalised by their count, so this thickens without getting louder. |
| `spread` | `12` | Cents between the outermost unison voices, centred on `detune_cents`; read only above one voice. `25` is a fat lead; past `50` the voices read as a chord. |
| `width` | `0` | How far the unison voices fan across the stereo field, `0..1` (clamped): `1` puts the outermost two hard left and right. Read only above one voice. |
| `pulse_width` | `0.5` | *square only* (refused on any other wave) — the share of each cycle spent high, `0.05..0.95` (clamped). `0.5` is the square; `0.25` and `0.125` are the other chiptune duties; narrower is quieter and thinner. |

#### Partials

One entry of an `additive` source's `partials`.

| Key | Default | Meaning |
|---|---|---|
| `ratio` | *required* | Frequency as a multiple of the played pitch: `1` the fundamental, `2` the octave. Fractions go inharmonic (a bell, a plate). At or below `0` is refused. |
| `gain` | `1` | Weight in the series, normalised by the sum of the gains of the partials that sound. |
| `detune_cents` | `0` | Detune off `ratio`, in cents: a few more cents on each partial than the last is a piano's stretched upper partials. |
| `decay` | `0` | Seconds for this partial's own exponential fade under the amp envelope; `0` does not fade. The two multiply, so a patch shaped by its partials wants an `amp` that sustains. |

#### Four-operator FM

`algorithm` is one of these routings (`→` is "modulates", `+` is "mixed"); the operators
at the right of each are the carriers, the ones heard.

| `algorithm` | Routing | Character |
|---|---|---|
| `chain` | `1 → 2 → 3 → 4` | The brightest and most extreme; goes inharmonic fastest. |
| `stack` | `(1 + 2) → 3 → 4` | Two modulators colouring one wave that shapes the carrier. |
| `branch` | `2 → 3`, `(1 + 3) → 4` | A plain partial and a complex one on the same tone. |
| `fork` | `1 → 2`, `(2 + 3) → 4` | `branch` renumbered. |
| `twin` | `(1 → 2) + (3 → 4)` | Two two-operator voices mixed: horns, electric pianos, most FM basses. |
| `fan` | `1 → (2, 3, 4)` | One modulator into three carriers: organ-ish, choral. |
| `pair_and_two` | `(1 → 2) + 3 + 4` | One FM voice over two plain sines. |
| `parallel` | `1 + 2 + 3 + 4` | Four sines, no modulation. |

| Key | Default | Meaning |
|---|---|---|
| `ratio` | *required* | Frequency as a multiple of the played pitch; fractions (`1.41`, `3.5`) go metallic. At or below `0` is refused. |
| `level` | `1` | A carrier's weight in the mix, or a modulator's modulation index (radians). A routing whose every carrier is at `0` is refused. |
| `feedback` | `0` | How much of its own output bends its own phase, `0..1` (clamped). |
| `env` | none | Its own [envelope](#envelopes-adsr) over the note; absent, full level throughout. |

#### Envelopes (ADSR)

The shape of `amp`, a filter's and a pitch envelope's `adsr`, and an operator's `env`.
A filter or pitch envelope left out is `a = 0.005, d = 0, s = 1, r = 0.05`.

| Key | Default | Meaning |
|---|---|---|
| `a` | *required* | Attack: seconds from silence to full level. |
| `d` | *required* | Decay: seconds from full level down to `s`. |
| `s` | *required* | Sustain: the level held while the note is, `0..1` (clamped). |
| `r` | *required* | Release: seconds back to silence once the gate closes, which is why a note rings past its `duration`. |
| `curve` | `0` | How the three timed segments bend: `0` straight, positive the exponential approach a struck or plucked thing makes, negative a slow start and sudden arrival (a swell). Clamped to `±8`. |

#### The filter

| Key | Default | Meaning |
|---|---|---|
| `kind` | *required* | `lowpass` (keeps below the cutoff: darker), `highpass` (keeps above: thinner, a small speaker), `bandpass` (a band around it: telephone, radio, wah) or `notch` (drops a band: a phaser's trick). |
| `cutoff` | *required* | Base cutoff in Hz, written for a note at middle C when `key_track` is on. At or below `0` is refused. |
| `slope` | `"12db"` | `12db` (one pole pair, gentle) or `24db` (two in series: the obviously-filtered sweep; resonance rings harder). |
| `resonance` | `0` | Emphasis at the cutoff, `0..1`; near 1 it self-rings, sooner at `24db`. |
| `env_octaves` | `0` | Octaves the filter's `adsr` opens the cutoff by at full level; negative sweeps down. |
| `vel_octaves` | `0` | Octaves a full-velocity strike opens the cutoff by (velocity plus the bake's `timbre`). |
| `key_track` | `0` | How far the cutoff follows the note, `0..1` (clamped): `1` moves it an octave per octave played. |
| `adsr` | the fast shape above | The cutoff envelope `env_octaves` scales. |

The cutoff a note hears is `cutoff × 2^(env_octaves × env + vel_octaves × velocity + lfo)`.
*(Before #413 these were `env_amount`/`vel_cutoff` in Hz; an old patch using them is
refused by name, not misread.)*

#### The pitch envelope

| Key | Default | Meaning |
|---|---|---|
| `semitones` | *required* | Semitones the envelope adds at full level. The played note is where the sweep **ends**: positive falls onto it from above (a kick, a tom, an 808), negative rises from below. |
| `adsr` | the fast shape above | The envelope doing the sweeping. That default sustains, so a sweep spells out `s = 0` and a decay. |

#### Layers

One entry of `layers`: a sound of its own, summed with the patch's before the patch's
one `fx` chain (a snare's crack over its body, a kick's click over its sine).

| Key | Default | Meaning |
|---|---|---|
| `source` | *required* | This layer's [source](#sources). |
| `amp` | *required* | This layer's amplitude envelope. |
| `filter` | none | Its own filter. A song's `cutoff` automation moves the patch's top-level filter only. |
| `pitch_env` | none | Its own pitch sweep. |
| `gain` | `1` | How loud it sits against the patch's own sound, linear; at or below `0` is refused. |

#### LFOs

`lfo` and `lfo2` each bend one target of every layer.

| Key | Default | Meaning |
|---|---|---|
| `rate` | — | Speed in Hz, typically `0.1..10`. Write `rate` **or** `beats`, one of them. |
| `beats` | — | One cycle's length in beats of the song (`1` once a beat, `0.5` twice), instead of `rate`: only in a patch a song plays. |
| `depth` | *required* | How far it swings, in its target's unit. |
| `target` | *required* | `pitch` (vibrato, `depth` semitones either way), `cutoff` (`depth` octaves either way), `amp` (tremolo: dips by `depth`, `1` to silence) or `pulse_width` (every `square` swings `depth` around its `pulse_width`; refused with no square). |
| `shape` | `"sine"` | `sine`, `triangle` (a siren), `saw_up`, `saw_down`, `square` (a trill, a gated tremolo) or `random` (a seeded sample-and-hold, new each cycle). |
| `delay` | `0` | Seconds into the note before it moves anything. |
| `fade` | `0` | Seconds after `delay` to grow to full depth: vibrato arriving on a held note. |

#### Effects

An `fx` list (a patch's, a [track's or the song's](#the-song)), applied in order; `fx`
names each one. The bake limiter is not one of them: it always runs.

| `fx` | Keys | What it is |
|---|---|---|
| `delay` | `time` or `beats`, `feedback`, `mix`, `ping_pong` | A feedback echo. |
| `reverb` | `size`, `damp`, `mix`, `predelay` | A stereo room (Freeverb). |
| `saturate` | `drive`, `mix` | A soft clip: the warmth and glue, and the one stage that adds harmonics. |
| `compress` | `threshold`, `ratio`, `attack`, `release`, `makeup`, `mix`, `sidechain` | Pushes the loud moments down, so a part sits. |
| `chorus` | `rate`, `depth`, `voices`, `mix` | Detuned, delayed copies spread across the stereo field: one source as a section. |
| `eq` | `bands` | Up to **8** [EQ bands](#eq-bands), in order: the fix for a muddy or thin part. |

| Key | Default | Meaning |
|---|---|---|
| `time` | `0` | *delay* — echo spacing in seconds. |
| `beats` | none | *delay* — echo spacing in beats (`0.75` is a dotted eighth), instead of `time` (both is refused). Only in a track's or the song's `fx`; a patch's is refused. |
| `feedback` | *required* | *delay* — how much of each echo feeds the next, `0..0.95` (clamped). |
| `mix` | *required* (`compress`: `1`) | Every effect — wet/dry, `0..1` (clamped). Below `1` a `saturate` or `compress` is parallel: a driven or squashed copy under a clean one. |
| `ping_pong` | `false` | *delay* — the repeats alternate left and right. |
| `size` | *required* | *reverb* — room size, `0..1` (clamped). |
| `damp` | *required* | *reverb* — how fast the tail dulls, `0..1` (clamped). |
| `predelay` | `0` | *reverb* — seconds before the room answers, up to `0.5` (clamped); `0.02..0.06` keeps an attack clear of it. |
| `drive` | *required* | *saturate* — how hard the signal is pushed: below `0.01` nothing, `1..2` warmth, `4` audible drive, capped at `8`. Gain-compensated, so it shapes and does not raise the peak. |
| `threshold` | *required* | *compress* — the level above which it pushes down, dBFS, `-60..0` (clamped). |
| `ratio` | *required* | *compress* — `n:1`: `2` gentle, `4` the workhorse, `10` pinned, `1` off; capped at `20`. |
| `attack` | `0.01` | *compress* — seconds to full reduction, `0.0005..2` (clamped). |
| `release` | `0.15` | *compress* — seconds to recover, `0.0005..2` (clamped); too short and the mix breathes. |
| `makeup` | `0` | *compress* — dB handed back to the compressed copy, `±24` (clamped). |
| `sidechain` | none | *compress* — another track by name whose part the detector listens to (the kick pressing the bass down). Only in a track's `fx`. |
| `rate` | *required* | *chorus* — sweep speed in Hz, `0.3..1.5` usual, capped at `10`. |
| `depth` | *required* | *chorus* — how far apart the copies are pushed, `0..1` (clamped): `0.3` a shimmer, `0.8` a wide ensemble. |
| `voices` | `3` | *chorus* — how many copies, `2..4` (clamped). Normalised, so it thickens without getting louder. |
| `bands` | *required* | *eq* — the bands. |

#### EQ bands

| Key | Default | Meaning |
|---|---|---|
| `kind` | *required* | `highpass` (everything below goes), `lowshelf` (everything below moves by `gain_db`), `peak` (a bump or dip `q` wide), `highshelf` (everything above moves: air) or `lowpass` (everything above goes). |
| `freq` | 250 Hz, or 4 kHz for `highshelf` and `lowpass` | Where it acts, in Hz: the corner, or a peak's centre. The defaults are the edges of the level report's `bands`, so a band answers what it diagnosed. |
| `gain_db` | `0` | dB added or taken away, `±24` (clamped); read by the shelves and `peak` only. |
| `q` | `0.707` | How narrow, `0.1..30` (clamped): `0.7` gentle, `2` a noticeable notch, `8` surgical. |

### Songs (#358)

A **song** is the same idea one level up: where a patch is one instrument, a song is a
piece of music — which instruments play (`tracks`), what they play (`patterns` of
notes), and in what order (`arrangement`). It bakes to a single stereo WAV the audio
runtime plays like any other clip.

| Function | Signature | Returns |
|---|---|---|
| `Sound.BakeSong` | `(song, path)` | the written `path`, then its level |
| `Sound.SongToJson` | `(song)` | the song's canonical JSON string |

`song`, like `patch`, is a table **or** its JSON string (from `Sound.SongToJson`).

The shape is **tracker-style** (the MOD/XM lineage), not a flat piano roll: patterns
are named blocks you list in the arrangement, so a piece that repeats stays short
enough to write, diff and iterate on by hand.

```lua no-run
local theme = {
  bpm = 120,
  seed = 7,
  tracks = {
    { name = "bass", patch = "assets/sounds/bass.json", gain = 0.8 },
    { name = "lead", patch = { source = { kind = "karplus", damping = 0.996,
                                          brightness = 0.5 },
                               amp = { a = 0.001, d = 0.3, s = 0.0, r = 0.2 } },
      gain = 0.6, pan = 0.3 },
  },
  patterns = {
    verse = { beats = 4, notes = {
      { track = "bass", note = "E2", start = 0.0, dur = 0.5 },
      { track = "lead", note = "B3", start = 2.0, dur = 1.0, vel = 0.8 },
    } },
  },
  arrangement = { "verse", "verse" },
}

local clip = Sound.BakeSong(theme, "assets/sounds/theme.wav")
Audio.PlayAt(clip, 0, 0, 0, 1.0)
```

Every key a song takes is in the tables below, read like the patch's.

#### The song

| Key | Default | Meaning |
|---|---|---|
| `bpm` | *required* | Tempo in beats per minute, above `0`: the one place beats become seconds, so retiming a song is one number. Where it starts, if `tempo` moves it. |
| `tempo` | none | [Tempo changes](#tempo-changes) after the first beat. |
| `seed` | `0` | Folded into every note's render seed: one number re-rolls every stochastic source in the piece. |
| `key` | none | The key, as a tonic and a [mode](#keys-and-degrees) (`"D minor"`, `"F# lydian"`): what lets a note be a `degree` and a section move by `transpose_degrees`. |
| `tracks` | *required* | The [instruments](#tracks), at least one. Their order is part of every note's seed. |
| `patterns` | *required* | Named [blocks of notes](#patterns-and-notes): an object of name → pattern. |
| `arrangement` | *required* | Which patterns play, in order: each entry a pattern's name, a [`play` object](#the-arrangement) or a `layers` object. |
| `swing` | `0` | How far each off-beat eighth sits behind the grid, `0..1`: `0` straight, `0.33` roughly triplet, `0.5` dotted. Moves onsets only. |
| `humanize` | none | How far the [player strays](#the-performance) in timing, velocity and brightness. |
| `fx` | none | [Effects](#effects) on the finished mix, before the master limiter: the room (one reverb over everything) and the glue. |
| `automation` | none | [Values that move](#automation) across the piece. |
| `fit` | none | A length the file must come out at ([below](#how-a-song-ends-tail-fit-fade-377)). |
| `anchors` | none | [Sections pinned to times](#anchors), the tempo moved between them so each lands. |
| `fade` | none | Level moves on the finished piece ([below](#how-a-song-ends-tail-fit-fade-377)). |
| `tail` | `"ring"` | What happens after the last beat ([below](#how-a-song-ends-tail-fit-fade-377)). |

#### Tracks

| Key | Default | Meaning |
|---|---|---|
| `name` | *required* | How notes, automation and a `sidechain` name this track. |
| `patch` | *required* | Either a **path** to a saved patch `.json` or an **inline patch table**: the same duality `Sound.Bake`'s `patch` argument has. A path is read as-is, relative to the working directory. |
| `gain` | `1` | Linear mix level: a balance control, not a safety one (the master limiter always runs). |
| `pan` | `0` | Stereo position, `-1` (left) to `1` (right), clamped. |
| `fx` | none | [Effects](#effects) on this track's whole part, after each note's own patch `fx` and before `gain`: a delay that belongs to the keys rather than to one chord. |
| `send` | `1` | How much of the part reaches the song's `reverb` and `delay`, `0..1` (clamped): `0` keeps it dry in a wet mix. |
| `push` | `0` | Seconds this part sits off the beat: positive lays it back, negative pushes ahead (`0.015` on a snare, `-0.008` on a hat). |
| `swing` | the song's | This part's own `swing`, in place of the song's: a shuffled hat over a straight kick. |
| `humanize` | the song's | This part's own `humanize`, whole, in place of the song's. |

#### Patterns and notes

A pattern is `{ beats, notes }`; each entry of `notes` is written in one of four
forms, told apart by the key it carries: `note`, `degree`, `chord` or `steps`.

```lua no-run
patterns = { groove = { beats = 4, notes = {
  { track = "bass", note = "E2", start = 0, dur = 0.5 },             -- a note
  { track = "lead", degree = "b3", oct = 4, start = 1, dur = 1 },    -- a degree of the key
  { track = "keys", chord = "Am7", start = 0, dur = 4, arp = "up", div = 0.25 },
  { track = "hat",  steps = "x-x-x-x-x-xXx-x-", div = 0.25, vel = 0.5 },
} } }
```

| Key | Default | Meaning |
|---|---|---|
| `beats` | *required* | How long the block occupies in the arrangement, above `0`. Notes may ring out past it; the next pattern still starts on time. |
| `notes` | *required* | What is played, and when within the block. |
| `track` | *required* | Every form — the `name` of the track that plays it. |
| `note` | *required* | *a note* — a name (`"C#4"`) or a MIDI number, as `Sound.Bake` takes. On a *steps* entry, the pitch of every hit, middle C when absent. |
| `start` | *required* (steps: `0`) | Onset **in beats** from the start of the pattern; on a *steps* entry, of its first step. |
| `dur` | *required* (steps: one `div`) | Gate length **in beats**, above `0`; the patch's release rings out after it. |
| `vel` | `1` | Velocity, `0..1`. On *steps*, a plain `x`'s. |
| `articulation` | none | How it is played (below); on a chord every voice, on steps every hit. |

**Articulations** are the mark a score writes over a note:

| `articulation` | Effect |
|---|---|
| `accent` | Struck harder (velocity ×1.3) and brighter: the note that carries the groove. |
| `staccato` | Sounds for half its `dur`. |
| `ghost` | Quiet (velocity ×0.35), short (×0.4), dull and 12 ms early: the note between the notes. |
| `glide` | Slides onto its pitch from the track's previous note, over 0.06 s or half its gate if shorter; a track's first note plays plain. |

#### Keys and degrees

`key` is a tonic (a letter, then `#` or `b` accidentals) and a mode, in any case:

| Mode | Scale |
|---|---|
| `major` / `ionian` | The major scale. |
| `dorian` | Minor with a raised sixth. |
| `phrygian` | Minor with a flattened second. |
| `lydian` | Major with a raised fourth. |
| `mixolydian` | Major with a flattened seventh. |
| `minor` / `aeolian` | The natural minor scale. |
| `locrian` | Minor with a flattened second and fifth. |

A *degree* entry plays a note of that scale (refused in a song with no `key`):

| Key | Default | Meaning |
|---|---|---|
| `degree` | *required* | Which degree, the tonic being `1` and `8` the octave above: a whole number, or a string with accidentals in front (`"b3"`, `"#4"`, `"bb7"`). |
| `oct` | `4` (degree), `3` (chord) | The octave the **tonic** sits in, so `1` in D minor at `4` is D4 and `7` is C5. On a *chord*, the octave of its root: `"Dm7"` at `3` is rooted on D3. |

#### Chords

A *chord* entry sounds several notes as one: all at once, or one at a time as an
arpeggio that fills its `dur`.

| Key | Default | Meaning |
|---|---|---|
| `chord` | *required* | A **name**, a root letter with accidentals then a quality from the table below (`"Dm7"`, `"F#maj9"`, `"Bb7"`), optionally over a slash bass (`"C/G"`, the bass under the root); or a **list of pitches** spelled out, low to high. A named chord is voiced close, from the root at `oct`. |
| `arp` | none | Play the voices one at a time: `up` (low to high, round again), `down`, or `up_down` (four voices are `1 2 3 4 3 2`). |
| `div` | — | *with `arp`, required* — one step of the figure in beats (`0.5` an eighth); refused without an `arp`. |
| `gate` | one `div` | *with `arp`* — each figure note's gate in beats; longer lets a sustaining patch pile up into the chord. Refused without an `arp`. |
| `inv` | root position | *a named chord* — its lowest voice raised an octave this many times (`"C"` at `1` is E G C); a slash bass stays under. Past the last inversion is refused. |
| `voicing` | root position | *a named chord* — `near`: the inversion, within an octave of root position, whose voices move least from the track's previous chord in this pattern. Not beside `inv`. |

The qualities, written on C, with each voice's semitones above the root:

| Quality | Semitones |
|---|---|
| `C` / `Cmaj` | 0 4 7 |
| `Cm` / `Cmin` | 0 3 7 |
| `Cdim` | 0 3 6 |
| `Caug` | 0 4 8 |
| `Csus2` | 0 2 7 |
| `Csus4` | 0 5 7 |
| `C5` | 0 7 |
| `C6` | 0 4 7 9 |
| `Cm6` | 0 3 7 9 |
| `C7` | 0 4 7 10 |
| `Cmaj7` | 0 4 7 11 |
| `Cm7` / `Cmin7` | 0 3 7 10 |
| `Cmmaj7` | 0 3 7 11 |
| `Cm7b5` | 0 3 6 10 |
| `Cdim7` | 0 3 6 9 |
| `Caug7` | 0 4 8 10 |
| `C7sus4` | 0 5 7 10 |
| `C7b5` | 0 4 6 10 |
| `C7b9` | 0 4 7 10 13 |
| `C7#9` | 0 4 7 10 15 |
| `Cadd9` | 0 4 7 14 |
| `Cmadd9` | 0 3 7 14 |
| `C9` | 0 4 7 10 14 |
| `Cmaj9` | 0 4 7 11 14 |
| `Cm9` | 0 3 7 10 14 |

#### Steps

A *steps* entry is a run of evenly spaced hits written as one string over a grid: a
drum part.

| Key | Default | Meaning |
|---|---|---|
| `steps` | *required* | One character a step: `x` a hit at `vel`, `X` an accent at velocity `1`, `o` a ghost, `-` a rest. It must fill the pattern from `start` to its end exactly. An `x` and an `X` in one string need a `vel` below `1`. |
| `div` | *required* | One step's length in beats: `0.5` an eighth, `0.25` a sixteenth. |

#### The arrangement

Each `arrangement` entry is a pattern's name, a `play` object (a pattern and what to
change about it this time) or `{ layers = { … } }`, several of those sounding at once
from the slot's first beat, the slot as long as the longest.

| Key | Default | Meaning |
|---|---|---|
| `pattern` | *required* | Which pattern plays. |
| `transpose` | `0` | Semitones added to every note (fractions too); past the MIDI range is clamped. |
| `transpose_degrees` | `0` | Scale steps added to every note **within the song's key**: the lift. Needs a `key`; not beside `transpose`. |
| `vel_scale` | `1` | Multiplies every note's velocity: a quiet reprise. |
| `tracks` | every track | Play only these tracks, by name: drop to one instrument for eight bars. |
| `layers` | *required* | The names or `play` objects that sound together, at least one. |

#### Automation

One entry of `automation`: one parameter of one track moving across the piece.

| Key | Default | Meaning |
|---|---|---|
| `track` | *required* | The track it rides, by name. |
| `param` | *required* | `gain` (linear, the fader), `pan` (`-1..1`), `cutoff` (Hz, the patch's top-level filter; refused without one) or `send` (`0..1`; refused when the song's `fx` has no `reverb` or `delay`). |
| `points` | *required* | The control points, in ascending `beat` order. Before the first and after the last, the value holds. |
| `beat` | *required* | *a point* — beats from the start of the rendered piece (not of a pattern). |
| `value` | *required* | *a point* — what the parameter reads there, in its own unit. |
| `easing` | `"linear"` | *a point* — how the value travels to the next point: `linear`, `ease_in` (slow start), `ease_out` (slow arrival), `ease_in_out`, or `hold` (stay, then jump). |

#### Tempo changes

One entry of `tempo`: from this beat on, this tempo.

| Key | Default | Meaning |
|---|---|---|
| `beat` | *required* | Where it lands, in beats from the start; after `0` and after the change before it. |
| `bpm` | *required* | The tempo on that beat, above `0`. |
| `ramp` | `false` | `true` moves the tempo evenly from the previous point to this one; `false` holds and jumps. |

#### Anchors

One entry of `anchors`: a section pinned to a time, the tempo moved (at most a quarter
either way) so it lands. Not with `fit` in mode `loop`.

| Key | Default | Meaning |
|---|---|---|
| `section` | *required* | The arrangement entry, counted from `0`, whose first downbeat is pinned. Each anchor names a later one than the last. |
| `seconds` | — | Seconds into the piece that downbeat falls at. Write `seconds` or `clip`, one of them. |
| `clip` | — | The name of a clip whose start it lands on. `Sound.BakeSong` resolves no clip, so it refuses a song that uses one: write `seconds`. |

#### The performance

`humanize` (on the song or a track) strays from the page, every draw seeded by the
song's `seed`:

| Key | Default | Meaning |
|---|---|---|
| `timing` | `0` | The widest an onset is nudged either way, in seconds. |
| `velocity` | `0` | The widest a velocity is scaled either way, as a fraction: `0.08` plays each note at 92–108% of its written velocity. |
| `timbre` | `0` | The widest a note's brightness strays from its level, as a fraction: brighter or darker without louder. |

**Mixing is addition.** Every note is rendered independently through the patch layer
above and summed into the master at its start offset — no voice limit and no
voice-stealing, because this is a bake, not a real-time synth. The **master limiter
always runs** on the sum, so a dense arrangement cannot clip no matter what the track
gains say.

**Determinism.** Each note's seed is derived from the song seed through the same kind
of seeded integer hash, so the same song and seed bake a byte-identical WAV in any
process. A pattern played twice gets two different noise draws — a repeated snare is
not a photocopy — and both are stable.

Validation happens **before** any samples are produced: an arrangement naming an
undefined pattern, a note naming an undefined track, a non-positive `bpm` / `beats` /
`dur`, a song that would run past 600 seconds, or an unreadable patch path each raise a message saying exactly which one went
wrong, rather than rendering silence you would have to listen for.

### How a song ends: `tail`, `fit`, `fade` (#377)

A song's natural length is its arrangement **plus** however long the last note and the
reverb take to stop ringing. That is right for a sting played once and wrong for a
**music bed**: `AudioSource.loop` jumps back to sample 0 when the file ends, so the
ring-out either sits there as a gap every pass or, cut short, clicks at the loop point.
Three optional song fields say how the piece ends; all absent bakes exactly what it
always did.

```lua no-run
Sound.BakeSong({ bpm = 96, tail = "wrap", --[[ tracks, patterns, arrangement ]] },
               "assets/sounds/combat_bed.wav")
```

| Field | Values | Effect |
|---|---|---|
| `tail` | `"ring"` *(default)* | The file grows to fit the release and fx tail. For one-shots. |
| | `"exact"` | The file is exactly the arrangement's length; the tail is faded into the last beat. For music that butts against something. |
| | **`"wrap"`** | The file is exactly the arrangement's length and the ring-out is **summed back onto the start** — the last bar's reverb rings over the first, so the loop point is seamless by construction. **What a looping bed wants.** |
| `fit` | `{ seconds, mode }` | Makes the file exactly `seconds` long. `mode = "loop"` *(default)* repeats the arrangement, cutting mid-pass; `"once"` plays through and pads with silence; `"stretch"` moves `bpm` so a whole number of passes lands on it, refusing past a quarter either way (the error names the tempo it would have needed). |
| `fade` | `{ in_seconds, out_seconds }` | Level moves on the finished piece, after the master limiter. For a fade that belongs to the *music*; ducking one use of it stays the `AudioSource`'s job. |

| Key | Default | Meaning |
|---|---|---|
| `seconds` | — | *fit* — the length in seconds, above `0`. |
| `to` | none | *fit* — `"clip"`: as long as the clip that plays the song, instead of `seconds`. `Sound.BakeSong` resolves no clip, so it refuses a `fit` that uses it: write `seconds`. |
| `mode` | `"loop"` | *fit* — `loop`, `once` or `stretch`, as above. |
| `in_seconds` | `0` | *fade* — seconds from silence to full level at the start. |
| `out_seconds` | `0` | *fade* — seconds from full level to silence at the end. |

`exact` and `wrap` lengths are exact to the sample: 64 beats at 96 bpm is 1,764,000
frames. `wrap` is **refused** — before anything is written — when the tail is longer
than the loop (it would still be ringing next time round), alongside any `fade` (a dip
every pass), and alongside a `fit` other than `stretch` (only `stretch` lands on a whole
number of passes, so only it has a loop point).

### How loud it came out (#378)

**The agent cannot hear**, so every bake says how loud it came out. Each bake verb
returns a **level table** as its second value — the path stays first, so
`local clip = Sound.Bake(...)` is unchanged — and `Sound.Level` measures any clip on
disk the same way: an imported `.wav` / `.ogg` / `.mp3`, or an earlier bake you are
comparing against.

| Function | Signature | Returns |
|---|---|---|
| `Sound.Level` | `(path [, { sections = seconds }])` | the level table of the clip at `path`; an error naming the path if it does not decode. `sections` cuts the file on a grid of that many seconds instead of the 8-second one (a positive length, at most 10 000 rows; else an error) — how a [capture of the game's mix](Audio.md#capturing-the-mix) is read a moment at a time |

| Field | Meaning |
|---|---|
| `mean` | RMS level in dBFS — how loud it actually is. |
| `peak` | The loudest single sample, in dBFS. |
| `true_peak` | The loudest point *between* samples (dBFS) — what a resampler or encoder must reproduce; can exceed `peak`. |
| `crest` | `peak − mean` in dB: a large crest has dynamics, a small one is a wall. |
| `seconds` | Length of the clip. |
| `silent` | `true` when it makes no sound at all. `mean`/`peak`/`true_peak`/`crest` are then `nil`. |
| `clipping` | `true` when the true peak reaches or passes 0 dBFS. |
| `bands` | `{ low, mid, high }` — where the energy sits, as whole percentages summing to 100: below 250 Hz, 250 Hz–4 kHz, above 4 kHz. `nil` for a silence. |
| `correlation` | `-1..1`, how much of the signal both channels share: `1` is mono in a stereo file, **negative** cancels when folded to mono. `nil` for a mono clip or a silent channel. |
| `sections` | The clip **over time** — a list of rows with the fields above plus `label`, `from`, `to` (seconds). |
| `tracks` | The mix **track by track** — a list of rows with the fields above plus `name` and that track's own `sections`. |

```lua no-run
local clip, level = Sound.Bake(pistol, "C2", "assets/sounds/pistol.wav")
local step = Sound.Level("assets/sounds/footstep.wav")
print(("pistol %.1f dBFS, footstep %.1f dBFS"):format(level.mean, step.mean))
```

**A signal, never a gate.** There is no correct loudness — a gunshot is meant to be
hot, a distant ambience far down — so nothing here refuses or fails; `silent` and
`clipping` are notes, not errors. What it is for: two one-shots baked at the same
`velocity` can land 20 dB apart because their sources have different crest factors,
and this is how you find that out without listening. It does **not** judge taste —
no number here says a gunshot is satisfying — and it is not LUFS.

The measurement is zimmer's (`zimmer::level`), taken on the samples before they are
encoded, so it costs nothing extra. `Sound.Level` decodes with the engine's own clip
decoder and feeds the same meter, so a bake and `Sound.Level` of its file agree (up
to the 16-bit rounding the WAV adds).

### Over time, across the spectrum, track by track (#379)

One number for a whole bake hides three things, and the report carries each:

- **`sections` — when.** A song's rows are its **arrangement's** patterns, labelled
  with the pattern name, so a row reads "the second `combat` is the quiet one";
  anything without an arrangement (a one-shot, a file) is cut on an 8-second grid,
  or on the grid `Sound.Level`'s `sections` asks for, `label = nil`. Empty when there would be only one row — that row is the summary
  said twice.
- **`bands` — muddy or thin.** A shooter's mix is a frequency-allocation problem
  before it is a level problem: a weapon and the music both owning the mids reads
  muffled at every volume, and no level meter can see it.
- **`tracks` — who.** For a song of more than one track, one row per track,
  measured **post-gain** where the mixer sums them (what each track contributes, not
  what it sounds like alone). A track that never plays is a `silent` row, not a
  missing one. Each carries `sections` cut where the song's are, so the n-th entry
  and the n-th section row are the same stretch. Empty for a one-shot.

| Function | Signature | Returns |
|---|---|---|
| `Sound.Diff` | `(a, b)` | how clip `a` differs from clip `b`, both paths measured as `Sound.Level` does |

| Field | Meaning |
|---|---|
| `mean` / `peak` / `crest` | `a − b` in dB: negative means `a` is quieter (or flatter, for `crest`). |
| `bands` | `{ low, mid, high }` in percentage **points** — 10% → 20% is +10 points. |
| `correlation` | `a − b`; **positive means `a` is narrower** (the one field whose sign reads backwards). |
| `seconds` | `a − b` length. |
| `same` | `true` when nothing measurably moved — "is this the same audio", not "close enough". |

A field is `nil` when either side had nothing to compare (a silence has no level and
no balance); `nil` is not zero.

```lua no-run
local _, level = Sound.BakeSong(theme, "assets/sounds/theme.wav")
for _, t in ipairs(level.tracks) do
  print(("%-6s mean %.1f  low %d%%  mid %d%%  high %d%%"):format(
    t.name, t.mean or -math.huge, t.bands and t.bands.low or 0,
    t.bands and t.bands.mid or 0, t.bands and t.bands.high or 0))
end
local d = Sound.Diff("assets/sounds/theme.wav", "assets/sounds/theme.prev.wav")
print(("%.1f dB vs the previous bake"):format(d.mean))
```

All of it is the same **signal, never a gate** as the level above: nothing here
refuses a bake or fails a build. The figures are zimmer's (`zimmer::level`:
`Profile`, `Bands`, `Layer`, `Difference`); rusty only shapes them into tables.

### What a set is made of (#380)

Every number above looks at **one** bake. `Sound.Survey` looks at a **set** of patch
and song documents and counts what they are made of — so "all fourteen impact sounds
are `noise` under a lowpass between 700 and 900 Hz", or "the same karplus arp is the
loudest thing in all six cues", is something you can read off a table without anyone
listening. It reads the documents only: **no bake, no decode, no samples**.

| Function | Signature | Returns |
|---|---|---|
| `Sound.Survey` | `(paths)` | the survey table below |

`paths` is a list. Each entry is a **path** to a patch or song `.json`, or an
**inline document** — a table, or a JSON string (any string starting with `{`), the
same two forms every `Sound` verb takes. A document with `tracks` is a song;
anything else is a patch. Inline entries are named `#1`, `#2`, … by position.

| Field | Meaning |
|---|---|
| `patches` | One row per one-shot patch: `name`, `source` (the kind, e.g. `noise`), `filter` (its kind, `nil` without one), `cutoff` (Hz, `nil` without a filter), `sustain`. |
| `songs` | One row per song: `name`, `bpm` (the tempo it plays at), `seconds` (one pass of the arrangement), `register` (`{ low, high, names }`, MIDI, e.g. `names = "E1-A5"`), `loudest` (track name), and `tracks`. |
| `songs[].tracks` | Per track: `name`, `source`, `gain`, `cutoff`, `sustain`, `notes`, `median` (MIDI), `duty`, `density` (notes per second). |
| `skipped` | `{ name, error }` for each document that could not be read or parsed. The rest of the set is still surveyed. |
| `rollup` | Across the set: `patches`, `songs`, `sources` (per kind: `source`, `patches`, `songs`, `loudest`, and `cutoff = { low, high }` over its patches), `tempo = { low, high }`, `register`. **`nil` for a set of one** — a summary of one row repeats it. |

```lua no-run
local report = Sound.Survey({
  "assets/sounds/impact_wood.json",
  "assets/sounds/impact_metal.json",
  "assets/sounds/impact_dirt.json",
})
for _, row in ipairs(report.rollup.sources) do
  print(row.source, row.patches, row.cutoff and row.cutoff.low, row.cutoff and row.cutoff.high)
end
```

How to read the columns:

- **`loudest` is `gain × duty`, never the highest gain.** Percussion is written loud
  *because* it is short: a hat at gain 0.9 firing for a fifth of each beat does not
  lead a piece over an arp held throughout. It is a better proxy, not an ear.
- **`duty`** is the share of the piece a track is sounding for, `0..1` — the
  *union* of its notes, so a chord held for a bar is one bar of sound.
- **`median`** is the median pitch, so one octave leap does not move where a track
  sits; with an even count it is the upper middle note, never a pitch between two.
- **`sustain` is the amp envelope's sustain, not the source's.** A `karplus` string
  damps on its own and an `fm2` modulator decays on its own, so a patch can read
  `sustain = 0.4` and still be a decaying sound. Don't over-trust this one number.
- **What a track *is* versus what it *does*:** `source`, `gain` and `cutoff` describe
  the instrument; `sustain`, `notes`/`density` and register describe its role. Two
  different source kinds can play the same role and read as one instrument, so look
  at both halves.
- A song track whose patch path cannot be resolved keeps its row with `source`,
  `cutoff` and `sustain` absent, rather than failing the survey.

**It counts, and stops.** There is no score, no grade, no diversity number, and
nothing here can fail: six variations on one instrument is a legitimate thing to
write on purpose. It also makes no claim that a combination "sounds like" anything —
the table reports what the documents say. Song rows are zimmer's survey
(`zimmer::survey`); patch rows are rusty's, since zimmer leaves one-shots out.
