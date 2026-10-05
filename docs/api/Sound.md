## `Sound`

Agent-composed **procedural sound authoring** (#357): describe an instrument as a
*patch* and **bake** one note of it to a stereo `.wav`. That single note is the whole
one-shot SFX story — a gunshot, an impact, a footstep, a UI blip are each one
rendered note of a noise / Karplus / FM patch — and the returned path drops straight
into `Audio.PlayAt` or an `AudioSource`'s `clip`, so a sound the agent invented is
audible in the same script that made it.

**The synthesiser is zimmer**, from [scorsese](https://github.com/MatthewLacerda2/scorsese)
(#413) — an external crate, not a module of rusty. It is a git dependency pinned to
one commit in `Cargo.toml`, so rusty's bakes change only when someone moves that pin
on purpose (and `zimmer::SYNTH_VERSION` says whether they will). `Sound` is a thin
adapter over it: rusty turns the Lua table into zimmer's document, zimmer renders,
rusty writes the file. **The full patch and song vocabulary is zimmer's rustdoc** —
this page carries worked examples and the fields you reach for first; build the
reference locally from the pinned rev with `cargo doc -p scorsese-zimmer --open`.

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
`.json`) — both forms decode alike, with the same errors. `note` is either a **name** — a letter `A`–`G`, any accidentals (`#`/`s`
sharp, `b`/`f` flat), then the octave, e.g. `"C#4"`, `"Bb3"`, `"C-1"` — or a **MIDI
number** (`60` is middle C, `69` is A4 = 440 Hz; fractions are legal microtones).
Pitch is equal temperament: `f = 440 × 2^((midi − 69) / 12)`. A rejected patch, note
or option raises a Lua error naming what was wrong, and writes no file.

`opts` is optional, and so is every field in it; any other key is an error:

| Field | Default | Meaning |
|---|---|---|
| `duration` | `0.5` | Gate length in seconds — how long the note is *held*. The amp release rings out **after** it, and an fx chain adds its own tail, so the file is longer than this. |
| `velocity` | `0.8` | Striking force, `0..1`, scaling the note's peak amplitude. |
| `timbre` | `0.0` | How far this strike's *brightness* sits from its level, in velocity units: added to `velocity` only for the routings that read velocity as effort (a filter's `vel_octaves`, `fm2`'s `vel_index`), so a note can be brighter without being louder. |
| `glide` | none | `{ semitones, seconds }` — the note starts `semitones` away from its pitch (positive = above) and slides onto it over `seconds`. Both fields required. |
| `seed` | `0` | Seed for the stochastic sources (noise, the Karplus pluck, oscillator phases). |

### The patch shape

The signal path is **fixed** — `source → filter → amp envelope → fx`, with an
optional pitch envelope and one LFO tapping one target. You choose what fills each
stage, never how they connect; that is what makes a patch playable as a note.
`source` and `amp` are mandatory, the rest optional.

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

The stages at a glance (field-level detail is in zimmer's rustdoc, `zimmer::patch`):

- **`source`**, tagged by `kind`: `osc_stack` (up to 4 band-limited oscillators,
  `sine`/`triangle`/`saw`/`square`, each with optional unison `voices`/`spread`),
  `karplus` (plucked string), `noise` (`color` = `white`/`pink`/`brown`), `fm2`
  (2-operator FM), `fm4` (4-operator FM with per-operator envelopes) and `additive`
  (a series of partials).
- **`filter`** — `kind` ∈ `lowpass`/`highpass`/`bandpass`/`notch`, `cutoff` in Hz,
  `resonance`, `slope`, and its own `adsr`. Modulation is in **octaves**: the cutoff
  is `cutoff × 2^(env_octaves × env + vel_octaves × velocity + lfo)`.
  *(Before #413 these were `env_amount`/`vel_cutoff` in Hz; an old patch using them
  is refused by name, not misread.)*
- **`amp`** — the mandatory ADSR: `a`/`d`/`r` in seconds, `s` a level `0..1`.
- **`pitch_env`** — a pitch shape the instrument puts on every note (a kick's drop).
- **`lfo`** — one sine at `rate` Hz on one `target`: `pitch` (semitones), `cutoff`
  (octaves) or `amp` (tremolo).
- **`fx`** — applied in list order, tagged by `fx`: `delay`, `reverb` (stereo),
  `saturate`, `compress`, `chorus`, `eq`. The bake limiter is **not** listed here: it
  is not a choice.

Example — bake a gunshot and fire it where the shot happened:

```lua
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
> is decoded by the same `ClipCache` path an imported clip is. See
> `docs/api-faithfulness.md`.

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

```lua
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

The fields you reach for first (the rest — `key` and scale degrees, chords, step
strings, `swing`, `humanize`, track and song `fx`, `automation`, `tempo` changes,
arrangement transforms — are in `zimmer::song`'s rustdoc; `tail`, `fit` and `fade` have
their own section below):

| Field | Default | Meaning |
|---|---|---|
| `bpm` | — | Tempo. The one place beats become seconds, so retiming a finished song is one number. |
| `seed` | `0` | Folded into every note's render seed. One number re-rolls every stochastic source in the piece. |
| `tracks[].patch` | — | Either a **path** to a saved patch `.json` or an **inline patch table** — the same duality `Sound.Bake`'s `patch` argument has (table or JSON string). A path is read as-is, relative to the working directory. |
| `tracks[].gain` | `1.0` | Linear mix level for that track. A balance control, not a safety one — see the limiter below. |
| `tracks[].pan` | `0.0` | Stereo position, `-1` (left) to `1` (right). |
| `patterns[].beats` | — | How long the block occupies in the arrangement. Notes may ring out past it; the next pattern still starts on time. |
| `notes[].note` | — | A name (`"C#4"`) or a MIDI number, exactly as `Sound.Bake` takes. |
| `notes[].start` / `dur` | — | Onset and gate length **in beats**, measured from the start of the note's own pattern. |
| `notes[].vel` | `1.0` | Velocity, `0..1`. |

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
`dur`, or an unreadable patch path each raise a message saying exactly which one went
wrong, rather than rendering silence you would have to listen for.

### How a song ends: `tail`, `fit`, `fade` (#377)

A song's natural length is its arrangement **plus** however long the last note and the
reverb take to stop ringing. That is right for a sting played once and wrong for a
**music bed**: `AudioSource.loop` jumps back to sample 0 when the file ends, so the
ring-out either sits there as a gap every pass or, cut short, clicks at the loop point.
Three optional song fields say how the piece ends; all absent bakes exactly what it
always did.

```lua
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
| `Sound.Level` | `(path)` | the level table of the clip at `path`; an error naming the path if it does not decode |

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

```lua
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
  `label = nil`. Empty when there would be only one row — that row is the summary
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

```lua
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

```lua
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
