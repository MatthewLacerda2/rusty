## `Audio`

Play sound through the engine's `AudioMaestro` (the audio engine singleton). An
entity carries an `AudioSource` component (`clip`, `volume`, `loop`,
`play_on_start`, `is_time_scaled`, plus the spatial fields `spatial_blend`,
`initial_distance`, `final_distance`, and `output_group`, the mixer group it plays
through); these verbs start/stop and retune it, fire one-shots, set the single master
volume and the speaker mode, read a source's resolved 3D spatial state, and drive the
[mixer](#mixer-groups-snapshots-and-ducking): groups (buses) with volume, filters and
a reverb send, snapshots, and ducking. Decode is `.ogg` (Vorbis) / `.wav` / `.mp3`, path-cached;
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

| Function | Signature | Returns |
|---|---|---|
| `Audio.Play` | `(id)` | `bool` — started the entity's `AudioSource` (logs a Play event) |
| `Audio.Stop` | `(id)` | — (stops the entity's voice, logs a Stop event) |
| `Audio.SetVolume` | `(id, v)` | — (retunes the live voice's volume, pre-master; no-op if not playing) |
| `Audio.PlayAt` | `(path, x, y, z [, vol [, min_distance, max_distance [, group]]])` | `bool` — fire-and-forget one-shot at a world position (`vol` defaults to 1.0; `nil` keeps the default); fully 3D and time-scaled (see below); the optional rolloff band (both or neither, `0 <= min_distance <= max_distance`, finite — else an error) replaces the default 1 → 16 m so a gunshot or explosion carries further; `group` is the mixer group it plays through (Master by default; an unknown name plays through Master with a warning); leaves no component, logged as a `PlayAt` event |
| `Audio.GetMasterVolume` | `()` | `number` (linear, `[0, 1]`) |
| `Audio.SetMasterVolume` | `(v)` | — (clamped to `[0, 1]`; re-folds every live voice) |
| `Audio.GetSpeakerMode` | `()` | `string` — `"headphones"`, `"tv"` or `"home_theater"` (the default) |
| `Audio.SetSpeakerMode` | `(mode)` | — (one of the names above, case-insensitive; anything else is an error). Persisted; see [Speaker mode](#speaker-mode) |
| `Audio.GetSpatial` | `(id)` | `(gain, pan, playing)` — the source's resolved 3D state against the listener (the active camera): `gain` linear pre-master, `pan` `[-1, 1]` (left→right), `playing` bool |
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
| `Audio.DefineSnapshot` | `(name, { [group] = { field = value, … }, … })` | — stores a snapshot (replacing one of that name). Fields: `volume`, `mute`, `low_pass`, `low_pass_resonance`, `high_pass`, `high_pass_resonance`, `reverb_send`; a field it leaves out is left alone by the transition. Unknown groups or fields, and a resonance without its cutoff, are errors |
| `Audio.TransitionToSnapshot` | `(name, seconds)` | — blends every group toward the snapshot over `seconds` of unscaled sim time (`0` lands at once); an unknown snapshot is an error |
| `Audio.GetSnapshot` | `()` | `(name, progress)` — the snapshot last transitioned to and how far along the blend is (`0..1`), or `nil` before any |
| `Audio.AddDuck` | `(trigger, target, volume [, attack [, release]])` | — while `trigger` (or a group under it) has a live voice, `target`'s gain ramps to `volume` (linear, `[0, 1]`) over `attack` seconds (default 0.1), and back to 1 over `release` seconds (default 0.5) once it is silent; unknown groups are an error |
| `Audio.ClearDucks` | `()` | — drops every duck rule; ducked groups recover at once |

A voice's volume is its per-source `volume` multiplied by the master volume (and by
its distance rolloff — see below). The play events are also visible in the `Debug.Snapshot` per-entity `audio` block (the
component's authoring fields).

### 3D spatialization

The **listener is the active camera**. Each `AudioSource` resolves to a per-source
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
active camera and hands the device its gain (× master), pan, rate and pause state.
`Audio.GetSpatial` and the device mix come from the **same** resolve call, so the
read-back is what plays. A voice started mid-frame is resolved against the previous
frame's listener, so its first samples are already spatialized. Pan changes are
applied in place — a moving source never restarts.

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

### Time scale and pause

`is_time_scaled = true` (the default — gameplay sound) makes a voice follow the clock:
it plays at rate `Time.timeScale` (slow-mo lowers the pitch with the rate — that is
the intended bullet-time sound), and it is **paused** while `Time.timeScale == 0` or
`Time.Pause()` is on, resuming where it left off. `is_time_scaled = false` (music, UI)
ignores both and always plays at normal rate. `Audio.PlayAt` one-shots are
time-scaled.

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
explosion. It is **output shaping only**: the simulation never reads the mode, so
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
A child sums into its parent, so a voice is scaled by its group's volume and every
ancestor's. Group names are exact (case-sensitive).

Each group has a **volume** and **mute**, a **low-pass** and a **high-pass** filter
(cutoff in Hz plus resonance; a 22 kHz low-pass and a 10 Hz high-pass are open and
change nothing), and a **reverb send**: how much of the group feeds the one reverb
bus (off by default; reverb zones, #469, will tune it).

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
trigger a duck there.

**Stop** discards play-mode mixer changes (groups created, settings, snapshots,
ducks), as it does the scene's. The device applies group changes with a 10 ms ramp,
so a blend stepped once a frame never clicks.
