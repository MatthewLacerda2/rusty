## `Audio`

Play sound through the engine's `AudioMaestro` (the audio engine singleton). An
entity carries an `AudioSource` component (`clip`, `volume`, `loop`,
`play_on_start`, `is_time_scaled`, plus the spatial fields `spatial_blend`,
`initial_distance`, `final_distance`); these verbs start/stop and retune it, fire
one-shots, set the single master volume, and read a source's resolved 3D spatial
state. Decode is `.ogg` (Vorbis) / `.wav` / `.mp3`, path-cached; a clip that fails to
decode logs one warning naming the file and plays nothing.

`Play`/`Stop`/`PlayAt` return a `bool` that is `true` when the maestro accepted the
voice; on the **headless harness the audio backend is a no-op**, so playback makes no
sound but every action is still recorded in the maestro's introspection log (so a
play-test can assert *what* played, *where*, and *by whom* — one-shots included).

| Function | Signature | Returns |
|---|---|---|
| `Audio.Play` | `(id)` | `bool` — started the entity's `AudioSource` (logs a Play event) |
| `Audio.Stop` | `(id)` | — (stops the entity's voice, logs a Stop event) |
| `Audio.SetVolume` | `(id, v)` | — (retunes the live voice's volume, pre-master; no-op if not playing) |
| `Audio.PlayAt` | `(path, x, y, z [, vol [, min_distance, max_distance]])` | `bool` — fire-and-forget one-shot at a world position (`vol` defaults to 1.0; `nil` keeps the default); fully 3D and time-scaled (see below); the optional rolloff band (both or neither, `0 <= min_distance <= max_distance`, finite — else an error) replaces the default 1 → 16 m so a gunshot or explosion carries further; leaves no component, logged as a `PlayAt` event |
| `Audio.GetMasterVolume` | `()` | `number` (linear, `[0, 1]`) |
| `Audio.SetMasterVolume` | `(v)` | — (clamped to `[0, 1]`; re-folds every live voice) |
| `Audio.GetSpatial` | `(id)` | `(gain, pan, playing)` — the source's resolved 3D state against the listener (the active camera): `gain` linear pre-master, `pan` `[-1, 1]` (left→right), `playing` bool |

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
