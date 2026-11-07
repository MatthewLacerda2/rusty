## `Audio`

Play sound through the engine's `AudioMaestro` (the audio engine singleton). An
entity carries an `AudioSource` component (`clip`, `volume`, `loop`,
`play_on_start`, `is_time_scaled`, plus the spatial fields `spatial_blend`,
`initial_distance`, `final_distance`, `output_group`, the mixer group it plays
through, `occlusion_enabled` and `doppler_level`); these verbs start/stop and retune it, fire
one-shots, set the single master volume and the speaker mode, read a source's
resolved 3D spatial state, tune [occlusion](#occlusion), and drive the
[mixer](#mixer-groups-snapshots-and-ducking): groups (buses) with volume, filters,
a reverb send and [effects](#group-effects), snapshots, and ducking, and read back the listener's
[reverb zones](#reverb-zones). Decode is `.ogg` (Vorbis) / `.wav` / `.mp3`, path-cached;
a clip that fails to decode logs one warning naming the file and plays nothing.

**Clip formats.** Ship **WAV** for one-shots and loops, **OGG** (Vorbis) for music and
long dialogue, where size matters (a 3-minute stereo track is ~30 MB as WAV, ~3 MB as
OGG; Vorbis loops without padding). **MP3 is a source format, not a shipped one**: an
`.mp3` that enters the project is converted to a 16-bit PCM `.wav` beside it, with the
encoder delay and padding trimmed, and the `.mp3` is removed (see
[`Assets.Refresh`](Assets.md#assetsrefresh)). So one-shots fire on their first real
sample and loops have no silence at the seam. Channels and sample rate are kept as the
source has them (a mono MP3 stays mono, a stereo one stereo; the stereo rule under
`spatial_blend` applies as for any clip). A `clip` that still names a converted
`foo.mp3` plays `foo.wav`. An MP3 not converted yet still plays, trimmed the same way,
until the next refresh replaces it.

`Play`/`Stop`/`PlayAt` return a `bool` that is `true` when the maestro accepted the
voice; on the **headless harness the audio backend is a no-op**, so playback makes no
sound but every action is still recorded in the maestro's introspection log (so a
play-test can assert *what* played, *where*, and *by whom* — one-shots included).
Dev builds read it back with `Debug.AudioEvents` and the live per-source roster with
`Debug.AudioVoices` (see `Debug.md`), and record what reached the output with a
[capture](#capturing-the-mix).

| Function | Signature | Returns |
|---|---|---|
| `Audio.Play` | `(id)` | `bool` — started the entity's `AudioSource` (logs a Play event) |
| `Audio.Stop` | `(id)` | — (stops the entity's voice, logs a Stop event) |
| `Audio.SetVolume` | `(id, v)` | — (Unity `AudioSource.volume`: sets the entity's `AudioSource` volume, clamped to `[0, 1]` as the inspector's is, and retunes its live voice, pre-master; no-op without an `AudioSource`) |
| `Audio.GetVolume` | `(id)` | `number` — the `AudioSource`'s volume, `[0, 1]` (`0` without one) |
| `Audio.PlayAt` | `(path, x, y, z [, vol [, min_distance, max_distance [, group]]])` | `bool` — fire-and-forget one-shot at a world position (`vol` defaults to 1.0; `nil` keeps the default); fully 3D and time-scaled (see below); the optional rolloff band (both or neither, `0 <= min_distance <= max_distance`, finite — else an error) replaces the default 1 → 16 m so a gunshot or explosion carries further; `group` is the mixer group it plays through (Master by default; an unknown name plays through Master with a warning); leaves no component, logged as a `PlayAt` event |
| `Audio.GetMasterVolume` | `()` | `number` (linear, `[0, 1]`) |
| `Audio.SetMasterVolume` | `(v)` | — (clamped to `[0, 1]`; re-folds every live voice) |
| `Audio.GetSpeakerMode` | `()` | `string` — `"headphones"`, `"tv"` or `"home_theater"` (the default) |
| `Audio.SetSpeakerMode` | `(mode)` | — (one of the names above, case-insensitive; anything else is an error). Persisted; see [Speaker mode](#speaker-mode) |
| `Audio.GetSpatial` | `(id)` | `(gain, pan, playing, occlusion)` — the source's resolved 3D state against the listener (the main view): `gain` linear pre-master (occlusion muffle included), `pan` `[-1, 1]` (left→right), `playing` bool, `occlusion` the smoothed [occlusion](#occlusion) factor `[0, 1]` (`0` when not playing) |
| `Audio.SetOcclusionSettings` | `({ strength, layer_mask, voices_per_tick })` | — changes the fields the table names: `strength` `[0, 1]` (default 1; 0 turns occlusion off), `layer_mask` (bit per layer, default all), `voices_per_tick` (≥ 1, default 16). An unknown key is an error |
| `Audio.GetOcclusionSettings` | `()` | table — `strength`, `layer_mask`, `voices_per_tick` |
| `Audio.SetOcclusionEnabled` | `(id, bool)` | — sets the entity's `AudioSource.occlusion_enabled` (default `true`); an entity with no `AudioSource` is an error |
| `Audio.GetOcclusionEnabled` | `(id)` | `bool` — the source's `occlusion_enabled`, or `nil` with no `AudioSource` |
| `Audio.SetDopplerLevel` | `(id, level)` | — sets the entity's `AudioSource.doppler_level` (default `1`; `0` off, above `1` exaggerated; a negative or non-finite value stores `0`); an entity with no `AudioSource` is an error. See [Doppler](#doppler) |
| `Audio.GetDopplerLevel` | `(id)` | `number` — the source's `doppler_level`, or `nil` with no `AudioSource` |
| `Audio.GetDoppler` | `(id)` | `number` — the doppler rate the source's voice plays at as of the last tick (`343/293` for one closing at 50 m/s); `1` when not playing |
| `Audio.SetSpeedOfSound` | `(c)` | — the speed of sound doppler resolves with, m/s, until Stop puts the project's back; not finite and positive is an error |
| `Audio.GetSpeedOfSound` | `()` | `number` — the speed of sound in force (default 343) |
| `Audio.SetDefaultSpeedOfSound` | `(c)` | — the project's speed of sound, `project.rusty`'s `"audio"` (the Build Settings window's *Speed of sound*): written to the file in the editor and a session, in memory only in the player and `play`; entering Play and Stop apply it. Not finite and positive is an error |
| `Audio.GetDefaultSpeedOfSound` | `()` | `number` — the project's speed of sound (default 343) |
| `Audio.SetOutputGroup` | `(id, group)` | — sets the entity's `AudioSource.output_group` (`""` is Master); an unknown group, or an entity with no `AudioSource`, is an error. Takes effect at the voice's next `Play` |
| `Audio.GetOutputGroup` | `(id)` | `string` — the source's `output_group` (`""` is Master), or `nil` with no `AudioSource` |
| `Audio.GetGroups` | `()` | `{string}` — every mixer group's name, parents before children (`Master` first) |
| `Audio.CreateGroup` | `(name [, parent])` | — adds a group under `parent` (default `Master`); a taken or empty name, or an unknown parent, is an error |
| `Audio.SetGroupVolume` | `(group, v)` | — linear, clamped to `[0, 1]` |
| `Audio.SetGroupMute` | `(group, bool)` | — |
| `Audio.SetGroupLowPass` | `(group, cutoff [, resonance])` | — cutoff in Hz, clamped to `[10, 22000]` (22000 is open); resonance `[0, 1]`, default 0 |
| `Audio.SetGroupHighPass` | `(group, cutoff [, resonance])` | — as `SetGroupLowPass`; 10 Hz is open |
| `Audio.SetGroupReverbSend` | `(group, v)` | — linear send level to the reverb bus, `[0, 1]` (0 sends nothing) |
| `Audio.GetGroupState` | `(group)` | table — `name`, `parent` (`nil` for Master), `volume`, `mute`, `low_pass`, `low_pass_resonance`, `high_pass`, `high_pass_resonance`, `reverb_send`, `duck` (the gain ducking applies now, 1 = none) and `effective_volume` (the group's and every ancestor's volume × mute × duck, before the master volume) |
| `Audio.SetGroupEffects` | `(group, { { kind = …, param = value, … }, … })` | — replaces the group's [effects](#group-effects) with this ordered list (at most 8; `{}` removes them all). A parameter left out takes its default. An unknown kind, parameter or sidechain group, a value out of its range, or a NaN is an error |
| `Audio.GetGroupEffects` | `(group)` | `{table}` — the group's effects in order, each with `kind` and every parameter (the EQ's `bands` nested; a `sidechain` only when set) |
| `Audio.DefineSnapshot` | `(name, { [group] = { field = value, … }, … })` | — stores a snapshot (replacing one of that name). Fields: `volume`, `mute`, `low_pass`, `low_pass_resonance`, `high_pass`, `high_pass_resonance`, `reverb_send`, and `effects` (a list, slot by slot, of `{ kind = …, param = value }`: the kind the group has in that slot now, and the parameters to blend, `{}` to skip a slot; a sidechain cannot be changed); a field it leaves out is left alone by the transition. Unknown groups or fields, and a resonance without its cutoff, are errors |
| `Audio.TransitionToSnapshot` | `(name, seconds)` | — blends every group toward the snapshot over `seconds` of unscaled sim time (`0` lands at once); an unknown snapshot is an error |
| `Audio.GetSnapshot` | `()` | `(name, progress)` — the snapshot last transitioned to and how far along the blend is (`0..1`), or `nil` before any |
| `Audio.AddDuck` | `(trigger, target, volume [, attack [, release]])` | — while `trigger` (or a group under it) has a live voice, `target`'s gain ramps to `volume` (linear, `[0, 1]`) over `attack` seconds (default 0.1), and back to 1 over `release` seconds (default 0.5) once it is silent; unknown groups are an error |
| `Audio.ClearDucks` | `()` | — drops every duck rule; ducked groups recover at once |
| `Audio.GetReverbState` | `()` | table — the listener's blended [reverb zones](#reverb-zones) as of the last `LateUpdate`: `decay_time`, `pre_delay`, `damping`, `wet` (what the reverb bus runs at), `weight` (the zones' summed weight at the listener, capped at 1; `0` in no zone) and `zones` (how many reach the listener) |

A voice's volume is its per-source `volume` multiplied by the master volume (and by
its distance rolloff — see below). The play events are also visible in the `Debug.Snapshot` per-entity `audio` block (the
component's authoring fields).

### 3D spatialization

The **listener is the main view**: the scene's main camera entity, or the free view
camera when there is none (see [`Camera`](Camera.md), *Which pose these move*). Each `AudioSource` resolves to a per-source
`(gain, pan)` from the listener and source world transforms (read back via
`Audio.GetSpatial`):

- **Distance rolloff** — full volume out to `initial_distance`, then a **linear**
  falloff across `initial_distance → final_distance`, silent at/beyond `final_distance`.
- **Pan** — the source direction projected onto the listener's right axis: a source to
  the left pans left (`-1`), to the right pans right (`+1`), dead ahead/behind is
  centred (`0`).
- **`spatial_blend`** — linearly lerps 2D ↔ 3D: `0` is pure 2D (full `volume`, centred —
  the non-spatialized default), `1` is fully spatialized, and intermediate values lerp
  each of gain and pan toward the spatial value.

The spatial math is pure and device-free (it resolves the same with or without an
audio device), so a headless play-test can assert that a gunshot to your left reads
back quieter and panned left.

**What reaches the speakers.** Every frame, after the sim advances, the windowed
runtime (editor and standalone player alike) re-resolves each live voice against the
listener and hands the device its gain (× master), pan, rate and pause state.
`Audio.GetSpatial` and the device mix come from the **same** resolve call, so the
read-back is the mix the device is given: the target it glides to, not the sample
playing at that instant (below). A voice started mid-frame is resolved against the previous
frame's listener, so its first samples are already spatialized. Pan changes are
applied in place — a moving source never restarts. The device glides a voice's gain,
pan, spatial blend and low-pass toward each frame's values per sample (a 10 ms time
constant, #1693), so a mix retuned once a frame is heard without steps whatever the
frame rate; a voice's first sample plays at its values, with no fade-in, and so does
the first sample after a pause. Rate and pause apply at once.

- **Pan law** — balance: the far channel fades linearly to silence, the near one stays
  at unity (`pan = -1` ⇒ right channel silent). A centred voice plays at full level on
  both channels.
- **Stereo clips** — a mono clip is copied to both channels, then panned. A stereo
  clip is lerped toward its mono downmix `(l + r) / 2` by `spatial_blend`, then
  panned: fully 3D (`1`) is downmixed then panned, pure 2D (`0`) keeps its stereo
  image untouched. Clips with more than two channels use their first two as L/R.
- **`PlayAt` one-shots** are diegetic: `spatial_blend = 1` and time-scaled, with the
  default `AudioSource` rolloff band (`initial_distance = 1`, `final_distance = 16`)
  unless the call passes its own `min_distance, max_distance` — the same **linear**
  rolloff, over the wider band. A gunshot 30 m away is silent by default and audible
  with `Audio.PlayAt(clip, x, y, z, 1.0, 2, 80)`; distant fire is where the fight is.

### Occlusion

A spatialized source (`spatial_blend > 0`, `occlusion_enabled`) heard through a
wall is **muffled**: quieter and low-passed, so a shot behind concrete sounds
unlike one in line of sight (Steam Audio's occlusion, cut down).

- **Casts.** Three rays go through the physics world from the listener (the main
  view) to the source: one at the source and one 0.5 m to each side of it, level,
  across the line of sight. The fraction blocked is the **occlusion factor**, so
  stepping past a door frame takes it through ⅓ and ⅔ instead of flipping it.
  Triggers pass sound, the collider the listener stands in is looked past, and so
  are the source's own entity and its parents. Only colliders on a layer in the
  `layer_mask` occlude: clear the bits of characters and props sound should pass.
- **Effect.** At factor `1` (× `strength`) the voice plays at 0.35 gain (about
  −9 dB) through a 12 dB/octave low-pass at 800 Hz; the cutoff moves in octaves
  between 22 kHz (open, the filter bypassed) and that. `Audio.GetSpatial`'s `gain`
  includes the muffle.
- **Cadence.** A voice is cast the moment it starts, so a shot behind a wall is
  muffled from its first sample. After that, each `LateUpdate` re-casts up to
  `voices_per_tick` voices, round-robin, so the cost stays bounded.
- **Smoothing.** The factor moves to each new cast linearly over 0.25 s of unscaled
  time, so a source never pops as you step around a corner.

All of it is resolved in the sim (deterministic, same on the headless harness as in
the editor); the device only applies the result. **Stop** resets the settings.
Material transmission, diffraction and portals are not modelled.

```lua with=radio:AudioSource
Audio.SetOcclusionSettings({ layer_mask = ~(1 << 8) })  -- all but layer 8 (characters)
Audio.SetOcclusionEnabled(radio, false)   -- this one carries through walls
```

### Time scale and pause

`is_time_scaled = true` (the default — gameplay sound) makes a voice follow the clock:
it plays at rate `Time.timeScale` (slow-mo lowers the pitch with the rate — that is
the intended bullet-time sound), and it is **paused** while `Time.timeScale == 0` or
`Time.Pause()` is on, resuming where it left off. `is_time_scaled = false` (music, UI)
ignores both and always plays at normal rate. `Audio.PlayAt` one-shots are
time-scaled.

### Doppler

A 3D source moving against the listener plays faster as they close and slower as
they part, so a rocket passing overhead whooshes (Unity's *Doppler Level*). The rate is

    rate = (c − v_listener·n) / (c − v_source·n)

with `n` the unit vector from the source to the listener and `c` the speed of
sound: at 50 m/s and `c = 343`, `343/293` (+17 %) coming and `343/393` (−13 %)
going. It multiplies the time-scale rate.

- **Level.** Both velocities are scaled by `doppler_level × spatial_blend`: a 2D
  source (`spatial_blend = 0`, the `AudioSource` default) never shifts, a fully 3D
  one shifts physically at level `1`, and a level of `2` doubles the effect. The
  inspector's *Doppler Level* is the same field.
- **Velocities** are each body's world-position motion over the tick, divided by
  the tick's scaled sim time: a Transform a script moves, a Rigidbody, a
  CharacterController and a source parented under any of them are all measured
  the same way. A `PlayAt` one-shot stays where it was fired; the listener's
  motion still shifts it.
- **No chirps.** The rate is clamped to `0.5..2` and the device glides it over the
  same ~10 ms as gain and pan; a body faster than sound plays at the clamp. A
  tick with no sim time (`Time.Pause`, `timeScale = 0`) keeps a time-scaled
  voice's rate, and plays an unscaled one (`is_time_scaled = false`) at `1`.
- **Jumps.** Play, Stop and a scene load forget the listener's motion, so the
  next tick hears it still. A body that moves faster than 1000 m/s in one tick is
  taken as a teleport and counts as still for that tick. A shorter jump (a
  respawn a few metres away) is not told apart from motion: it reads as velocity
  for that one tick (3 m toward the listener at 60 Hz is 180 m/s), clamped and
  smoothed by the glide.
- **Speed of sound.** A project setting: `project.rusty`'s
  `"audio": { "speed_of_sound": 343 }`, edited in the editor's *File ▸ Build
  Settings* window (*Speed of sound*, written when a drag or typed edit ends) or
  with `Audio.SetDefaultSpeedOfSound`. Entering Play and Stop apply the file's
  value; `Audio.SetSpeedOfSound` changes the running game's until Stop. A value in
  the file that is not a positive number (`"343"`, `null`, `-5`) keeps 343 with a
  warning, and the project still opens.

Output only: nothing in the sim reads a rate, so a replay is unchanged.

### Speaker mode

One global, player-facing setting that fits the final mix to the listening setup —
the "speaker mode" option of shipped games. The editor's **Config ▸ Audio / Speaker
mode** menu and `Audio.SetSpeakerMode` change the same value.

| Mode | What it does |
|---|---|
| `home_theater` *(default)* | The reference mix, untouched: full dynamic range, full stereo width. A game that never sets the mode sounds exactly as authored. |
| `headphones` | Narrower stereo image: every voice's pan is scaled by 0.6, so a hard-left source still reaches the right ear at 40 %, and a light crossfeed (15 %) narrows clips that are wide on their own. |
| `tv` | Dynamic range compression on the **summed** mix (a −24 dBFS, 4:1 compressor with +9 dB makeup, then a soft limiter): quiet cues such as footsteps come up, loud peaks are held down, nothing clips. |

The compression runs on the device's **output stage** (kira's main track) — the one
stage every voice sums into — because only the total can tell a footstep from a footstep under an
explosion. It runs **after Master's effects** ([Group effects](#group-effects)), so
Master's default limiter sees the mix first: `home_theater` passes what it lets
through bit for bit, so the −0.3 dBFS ceiling holds; `headphones`' crossfeed only
blends the two channels, so it holds there too; `tv`'s +9 dB makeup lifts the
limited mix again and its own soft limiter keeps it under full scale (0 dBFS), not
under Master's ceiling. It is **output shaping only**: the simulation never reads the mode, so
it cannot change a replay, and `Audio.GetSpatial` still reports the unshaped
`(gain, pan)`. On the headless harness the mode is kept and read back but there is
no device to shape. The mode persists in `Storage` as `audio.speaker_mode`, loaded
at startup and written back on quit. Stereo only: no surround or HRTF.

### Mixer: groups, snapshots and ducking

Unity's `AudioMixer`, cut down. Every voice plays through one **group** (a bus):
an `AudioSource` through its `output_group`, a `PlayAt` one-shot through its
`group` argument, both Master when unset. Groups form a tree; the mixer starts with
**`Master`** and five buses under it — **`Music`**, **`SFX`**, **`Voice`**,
**`World`**, **`UI`** — and `Audio.CreateGroup` adds more (`"Guns"` under `"SFX"`).
A child sums into its parent, so a voice's dry signal is scaled by its group's volume and every
ancestor's. Its reverb send is taken after its group's filters, effects and volume (a voice
played straight into `Master` sends before Master's, which run on the output), and the
wet signal comes back on the output, under Master's filters, volume and effects but past
every group between the two (a post-fader send that skips the ancestors, where Unity lets
the send's place be chosen). Group names are exact (case-sensitive).

Each group has a **volume** and **mute**, a **low-pass** and a **high-pass** filter
(cutoff in Hz plus resonance; a 22 kHz low-pass and a 10 Hz high-pass are open and
change nothing), and a **reverb send**: how much of the group feeds the one reverb
bus that [reverb zones](#reverb-zones) tune. **`World` sends 1 by default** (every
other group 0), so world sounds take on the room's reverb out of the box; give
`SFX` a send to make gunshots ring too.

**Snapshots** are named, *partial* group settings: a snapshot changes only the
fields it names, so a settings-menu `Music` volume survives a `BulletTime` that only
muffles `World`. `Audio.TransitionToSnapshot` blends from the current settings:
volume, resonance and send linearly, cutoffs in octaves (so a sweep sounds even);
`mute` switches as the blend starts. A `SetGroup*` call during a blend wins for the
fields it sets.

```lua
Audio.DefineSnapshot("Normal",     { World = { low_pass = 22000, volume = 1 } })
Audio.DefineSnapshot("BulletTime", { World = { low_pass = 900, volume = 0.8 } })
Audio.DefineSnapshot("Flashbanged", {
  World = { low_pass = 400, low_pass_resonance = 0.3, volume = 0.3 },
  Music = { volume = 0.2 },
})
Audio.TransitionToSnapshot("Flashbanged", 0)   -- the bang: at once
Audio.TransitionToSnapshot("Normal", 4)        -- the ears recover over 4 s
Audio.AddDuck("Voice", "Music", 0.3)           -- voice lines duck music
```

**Ducking**: `Audio.AddDuck(trigger, target, volume)` lowers `target` while any voice
plays in `trigger` or a group under it, with an attack and a release ramp.

**Time and determinism.** Blends and duck ramps advance on `Time.unscaledTime`,
stepped once per frame in `LateUpdate`: a blend keeps running through slow-mo and a
`timeScale = 0` pause menu, and freezes under the loop-level `Time.Pause()`. The mix
state is a function of sim time, so `Audio.GetGroupState` reads back the same values
on the headless harness (no device) as in the editor. A duck's trigger is "a voice is
live": on the harness one-shots end at once (nothing plays), so only entity sources
trigger a duck there. **A [capture](#capturing-the-mix) changes that:** once one
starts, the harness mixes for real and a one-shot stays live until it finishes
sounding, so it triggers its ducks too. `Audio.GetGroupState(...).duck` can then read
differently in a run recorded with `--record-audio` (or after `Harness.RecordAudio`)
than in the same run unrecorded.

**Stop** discards play-mode mixer changes (groups created, settings, effects,
snapshots, ducks), as it does the scene's; the mixer has no saved form, so a game
sets its effects from a script at start. The device applies group changes with a
10 ms ramp, so a blend stepped once a frame never clicks.

### Group effects

Unity's mixer effects and Godot's bus effects, a small fixed set: every group runs an
ordered list of up to 8 effects after its filters and before its volume, except
Master's (below).
A list is a Lua array: `Audio.SetGroupEffects("Master", { kind = "limiter" })`
(the outer braces forgotten), a list with a hole, or a table nested deeper than an
EQ's `bands` is an error naming the field, never a shorter list.
`Audio.SetGroupEffects` replaces the list; a snapshot's `effects` entry blends the
parameters of the effects already there, numbers linearly (frequencies in octaves)
and names at once, as it blends the volume. On the device every parameter glides
with the voices' 10 ms smoothing, so a retune never clicks; changing a slot's kind
starts a fresh effect there (a delay's echoes, a compressor's envelope start over).

**Master starts with a limiter at −0.3 dBFS**, so a pile-up of loud sounds never
takes the mix past it, the reverb included. It is an effect like any other: retune
it, or remove it with `Audio.SetGroupEffects("Master", {})`, and then nothing limits.
**Master runs on the whole mix**, as Unity's Master holds everything under it:
Master's filters, volume, mute and effects all run after the reverb bus (the wet
signal of every group's reverb send) has summed in, so a fade, a mute or a low-pass
on Master reaches every reverb tail too. Then the [speaker mode](#speaker-mode)
shapes what they let through. The order on the output: the groups under Master and
the reverb bus sum → Master's low-pass → its high-pass → its volume and mute → its
effects → the speaker mode's stage.

| Kind | Parameters (default) |
|---|---|
| `compressor` | `threshold` dB `[-60, 0]` (0: does nothing until set), `ratio` `[1, 48]` (4), `attack` ms `[0.1, 1000]` (50), `release` ms `[1, 5000]` (50), `makeup` dB `[0, 30]` (0), `sidechain` a group name (none) |
| `limiter` | `ceiling` dB `[-24, 0]` (−0.3), `pre_gain` dB `[-24, 24]` (0), `release` ms `[10, 3000]` (100) |
| `eq` | `bands`: six of `{ shape, frequency, gain, q }` — `shape` `"off"` (default), `"peak"`, `"low_shelf"` or `"high_shelf"`; `frequency` Hz `[20, 22000]` (32, 100, 320, 1000, 3200, 10000); `gain` dB `[-24, 24]` (0); `q` `[0.1, 10]` (0.707). A band left out stays off |
| `distortion` | `mode` `"soft"` (default), `"hard"` or `"bitcrush"`; `drive` `[0, 1]` (0: a straight wire); `pre_gain` dB `[-60, 60]` (0); `post_gain` dB `[-80, 24]` (0) |
| `delay` | `delay` ms `[10, 5000]` (500), `decay` `[0, 1]` (0.5: each echo feeds the next at half), `dry` `[0, 1]` (1), `wet` `[0, 1]` (1), `low_pass` Hz on the feedback `[20, 22000]` (22000: open) |

- **compressor**: above `threshold` the level is cut by `ratio` (4:1 lets 1 dB in 4
  through), measured on the louder channel's peak. With a **`sidechain`** it listens
  to that group instead: `sidechain = "SFX"` on `Music` ducks the music under every
  gunshot. The key is the other group's output peak after its volume, mute and duck
  (a muted `SFX` ducks nothing), as of the last block it published: see *Sidechain
  timing* below.
- **limiter**: a brickwall with an instant attack: no sample passes `ceiling`, and
  under it, once the gain has recovered from the last peak, the signal is untouched,
  bit for bit. A NaN or infinite sample comes out as silence.
- **eq**: each band an RBJ-cookbook biquad; an EQ with every band off changes nothing.
- **distortion**: *soft* is a `tanh` that keeps full scale at full scale, *hard* pushes
  the signal up to 25× and clips it flat at full scale, *bitcrush* quantises it to
  16 bits at no drive down to 1 bit at full drive.
- **delay**: Unity's *Echo*; the `low_pass` darkens each repeat. Each delay holds a
  5 s line (about 2 MB at 48 kHz), and keeps its group awake (below) until its last
  echo has played.

**Sidechain timing and cycles.** The device runs a group's children before the
group, but sibling groups in an order of its own: a compressor reads its key as the
key last published it, this block's peak if the key ran first and the previous
block's if not. So a sidechain may lag by one block (128 frames, 2.7 ms at 48 kHz),
the same way every time for the same mixer. A key the keyed group's own output
reaches — the group itself, one of its ancestors, or a loop of sidechains (`Music`
keyed by `SFX` and `SFX` by `Music`) — is a **cycle**: `SetGroupEffects` keeps the
compressor but drops that sidechain, with a warning in the console, and the
compressor listens to its own input.

**Cost.** Measured in a release build on one x86_64 core, per group per 128-frame
block (the callback's budget is 2.67 ms at 48 kHz): an empty chain 0.3 µs, a limiter
1.7 µs, a delay 1.5 µs, an EQ with all six bands on 4 µs, a compressor 5 µs, a
distortion 8.5 µs, and all five in one chain 20 µs, under 1 % of the budget. A
group with no effects only measures its peak for sidechains, and an effect sleeps,
with its group's filters, over a second of silence (a delay with an echo still to
play stays awake for it).

There is no mixer panel in the editor: groups and their effects are set through this
API.

```lua
Audio.SetGroupEffects("SFX", {
  { kind = "compressor", threshold = -18, ratio = 4, attack = 5, release = 120 },
  { kind = "eq", bands = { { shape = "low_shelf", frequency = 120, gain = 3 } } },
})
Audio.SetGroupEffects("Music", { { kind = "compressor", threshold = -30, ratio = 6, sidechain = "SFX" } })
Audio.CreateGroup("Guns", "SFX")
Audio.SetGroupEffects("Guns", {
  { kind = "distortion", mode = "soft", drive = 0.15 },
  { kind = "delay", delay = 90, decay = 0.2, wet = 0.25, low_pass = 3000 },  -- outdoor slapback
})
Audio.DefineSnapshot("Indoors", { Guns = { effects = { {}, { kind = "delay", wet = 0 } } } })
print(Audio.GetGroupEffects("Master")[1].kind)   -- limiter
```

### Reverb zones

Unity's `AudioReverbZone`: an entity with an
[`AudioReverbZone`](AudioReverbZone.md) is a sphere that sets the reverb of the
world mix while the listener (the main view) is inside it. The listener's
**weight** in a zone is 1 within its `min_distance`, 0 at or past its
`max_distance`, and linear between. Each `LateUpdate` the zones blend:

- the tail's **character** — `decay_time`, `pre_delay`, `damping` — is the
  weight-averaged character of the zones that reach the listener;
- its **level** — `wet` — is their weighted sum divided by `max(total weight, 1)`.
  A lone zone fades to dry at its edge; two zones the listener is fully inside
  average rather than double. An `Off` zone inside a `Hall` halves the hall's level.

Outside every zone the reverb bus is **silent**: a group's reverb send only colours
the mix where a zone says the space rings (outdoors, use no zone or an `Outdoor`
one). The blend runs on the zones' world positions in entity-id order, so it is a
pure function of the scene and the camera: `Audio.GetReverbState` reads back the
same values on the headless harness (no device) and in a replay. Inactive entities'
zones do not count. **Stop** dries the reverb.

On the device the reverb bus is a pre-delay into kira's reverb (Freeverb), fully wet:
`decay_time` sets the comb feedback, `damping` its damping, `wet` the bus's level,
and a 50 ms ramp smooths a jump (a teleport) into a glide. Geometry-derived
acoustics and convolution reverb are out of scope, as is a per-source send beyond
the groups'.

```lua
Audio.SetGroupReverbSend("SFX", 0.6)   -- gunshots ring in tunnels too
local r = Audio.GetReverbState()
print(r.decay_time, r.wet, r.zones)    -- e.g. 2.8  0.55  1 inside "B Tunnels"
```

### Capturing the mix

**The agent cannot hear**, and the event log says only *which* sounds played and
*when*. A capture writes the **final mix** to a WAV — every voice after gain, pan,
occlusion, its group's volume, filters and [effects](#group-effects), the reverb bus and the
[speaker mode](#speaker-mode)'s compressor and limiter, exactly what the device gets —
so "the sound gets choppy when I walk into the next room" becomes a file
[`Sound.Level`](Sound.md#how-loud-it-came-out-378) measures, before and after a fix.

| Function | Signature | Returns |
|---|---|---|
| `Audio.StartCapture` *(dev-only)* | `(path)` | — starts recording the final mix to a WAV at `path` (relative to the project); a capture already running, or a file that cannot be created, is an error |
| `Audio.StopCapture` *(dev-only)* | `()` | `string` — stops the capture and closes the file, returning its path; nothing running is an error |

The file is **32-bit float stereo**, so nothing is quantised and a peak past full
scale stays readable, at the device's rate in the windowed editor. With no device (a
scenario, a box with no sound card) the engine mixes **offline at 48 kHz**, in step
with the engine's time: a scenario's capture is a pure function of the run, and two
runs of one scenario write byte-identical files. Going offline restarts every
`AudioSource` that was playing from its first sample (one whose clip cannot be
decoded is dropped with a warning); after that the run keeps the real mixer, so a
one-shot stays live until it finishes sounding, and the ducks it triggers apply
([*Time and determinism*](#mixer-groups-snapshots-and-ducking): a recorded run's
`duck` can differ from an unrecorded one's). One capture runs at a time. In a scenario, `Harness.RecordAudio` /
`Harness.StopAudio` and `play --record-audio <path>` do the same with the file under
the run's output folder (`docs/testing.md`).

In the windowed editor the device's thread hands each block over without ever
waiting on the engine; a block the engine had no room for (a frame stalled for
seconds) is dropped, written as **silence of the same length in its place** so the
file's timing stays true, and `StopCapture` logs how many frames were lost. A capture
that cannot be written still closes over what reached the disk, and is the error. A
capture left running when the editor quits or a run fails is still closed as a
readable WAV.

**A stepped capture is silent.** While the clock is paused (`Time.Pause`), a
time-scaled source (the default, and every `PlayAt` one-shot) is held, and
`Time.Step` advances the world without un-pausing it: the headless session
(`session-mcp`) can only step, so a capture it takes holds the stepped length in
zeros. The capture says so: when every tick it ran was paused, `StopCapture` (and
`Harness.StopAudio`) warns *every tick of this capture ran paused*. To hear a run,
record a scenario (`play --record-audio`, `Harness.RecordAudio`), which ticks
unpaused.

```lua no-run
Audio.StartCapture("captures/door.wav")
-- … walk through the door …
local level = Sound.Level(Audio.StopCapture(), { sections = 0.25 })
```
