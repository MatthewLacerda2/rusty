## `Lighting`

**`Lighting.Generate()`** (#832) is Unity 5's *Generate Lighting* button: every bake a
scene has, in order, with the scene's own settings, and the result **written** — no
separate save. It bakes the lightmaps (`Lighting.BakeLightmaps`, below), then the light
and reflection probes (`Lighting.Bake`), so walls and actors are always lit by the same
bake, then the occlusion culling's PVS (*Occlusion culling*, below). The editor's **"Generate Lighting"** button (the scene inspector) is this exact
path. The call returns when the bake is done (Unity's `Lightmapping.Bake`); the button
bakes the same probes a step a frame — one reflection probe or a few light probes —
so the editor and the MCP keep answering (Unity's `BakeAsync`, #1602). Both say each
bounce pass and reflection probe in the console (`Baking reflection probe 3 of 12`).
**`Lighting.GenerateAsync()`** (#1999, Unity's `Lightmapping.BakeAsync`) presses that
button from a script: it starts the button's own job and returns at once, so prefer
it for a bake that may outlast the caller's patience, above all an MCP tool call
(Claude Code aborts a call that has sent nothing for 30 minutes, and a Dust2-sized
level bakes for longer). Poll **`Lighting.IsRunning()`** (Unity's
`Lightmapping.isRunning`) until it reads `false`, then **`Lighting.GetBakeStatus()`**
says how it ended (`done` with its summary, `failed` or `cancelled` with why, or
`refused` when it never started), and `Lighting.GetBakeReport(kind)` reads what it
wrote. It is refused, with an error, where the button is greyed out: during Play
(and once Play is asked for, `Editor.EnterPlay()` earlier in the line included),
while a bake runs (the button's, or a `GenerateAsync` not yet started), and on a
scene never saved. Only the editor window steps the job, so a host without one (the
headless `session` and `session-mcp` without `--attach`, the player, `play`) refuses
it with an error naming `Lighting.Generate()`, which bakes in the one call there.
The call only asks: the editor's next frame starts the job, within a frame of the
call. The job runs whether or not the window draws: with the screen locked or the
window minimised or covered, the editor still runs its frames, only without
painting, and steps the job on them, so a bake started from an agent runs to its
end. On macOS such a frame is skipped at once. On Linux it can wait up to a second
for the window before it is skipped, so the bake's probe stage runs slower there;
that the frames go on at all on a hidden Linux window rests on the GPU driver not
blocking, which has not been tried yet. The request belongs to
the scene it was made in, so `Scene.New()` / File ▸ New or a load before that frame
drops it and the status reads `refused` with why; it never bakes a scene you didn't ask about. It has no cancel; the card's Cancel button
stops the lightmap step.

**A bake and Play: whichever was asked for first wins** (#2042), and the other is
refused with a reason. Once Play is asked for, a bake is refused (above). While a
bake runs, or a `GenerateAsync` waits to start it (whenever `Lighting.IsRunning()`
reads `true`: Generate Lighting and Bake Occlusion alike), Play is refused: the Play
button is greyed out, its shortcut ignored, and its tooltip says why
(`Baking lighting: 42% — Cancel the bake to play`; the step's share once it counts
its work, `Baking lighting (probes)`, `(lightmap pages)` or `(occlusion)` for the
later steps, `Baking occlusion` for Bake Occlusion, and "wait for the bake to finish
to play" where the card has no Cancel, a `GenerateAsync` not yet started included), and
[`Editor.EnterPlay()`](Editor.md#play) errors with the same line. Cancelling,
finishing or failing the bake gives Play back. **Opening a demo is the exception**:
[`Editor.OpenDemo`](Editor.md#the-demo-timeline) is not
refused while a bake runs; the bake waits through the replay, as it waits through
Play, and resumes once the demo closes and the edit scene is back. Within one frame the order is the
order of the calls: `Lighting.GenerateAsync(); Editor.EnterPlay()` refuses the
`EnterPlay`, and `Editor.EnterPlay(); Lighting.GenerateAsync()` refuses the bake.
Against the Play button, the window's frame order decides: an attached agent's
command (`session-mcp --attach`) runs before the window draws, so a bake it asked
for greys out the button that frame; a line typed in the editor's console runs
after the draw, so a Play pressed that frame refuses its bake.
The two individual verbs stay for a script that wants only one of them. On a
`Shadowmask` scene (#1695) the lightmap bake records each masked light's channel and
only a probe bake writes the probes' occlusion on it, reading those channels from the
record and never assigning its own: so `Lighting.BakeLightmaps` warns once in the
console that the probes' occlusion is stale until `Probe.Bake()` or Generate
Lighting, and `Probe.Bake()` warns once when the record holds no channels (the
probes then carry no light's occlusion). Generate Lighting runs both, lightmaps
first.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.Generate` *(dev-only)* | `()` | `{ lightmaps, lightProbes, reflectionProbes, occlusionCells, occlusionObjects, occlusionBytes, occlusionNeverCulled, saved }` (the occlusion fields `0` when nothing occludes, or when the level is too large to cull; a PVS kept unchanged reports its own counts, *Occlusion culling*, below) |
| `Lighting.GenerateAsync` *(dev-only)* | `()` | `true` once the editor's Generate Lighting job is asked for; returns at once (*Generate Lighting*, above). Errors during Play or once it is asked for, while a bake runs, on an unsaved scene, and in a host with no editor window to step the job |
| `Lighting.IsRunning` *(dev-only)* | `()` | `true` while the editor's Generate Lighting or Bake Occlusion runs, or a `GenerateAsync` waits to start it |
| `Lighting.GetBakeStatus` *(dev-only)* | `()` | `{ state, message }`: the editor's last background bake (the button's, `GenerateAsync`'s or the bake queue's). `state` is `idle`, `requested`, `running`, `done`, `cancelled`, `failed` or `refused`; `message` is its summary or why, `nil` until it ended; while the editor's bake queue runs it says where the queue is (`scene 2 of 3: assets/scenes/tower-bar.scene, lighting (lightmap pages) 42%`) |
| `Lighting.GenerateScenes` *(dev-only)* | `(scenes)` | `{ scenes = { { path, state, reason, attempts, startedMs, finishedMs }, … }, failed }`: each scene baked in order, in this call (*Bake queue*, below). `scenes` is a list of project-relative paths, `"all"`, or `{ all = true, except = { … } }`; an ask that names no scene is refused, and an unfinished queue is left as it was |
| `Lighting.GenerateScenesAsync` *(dev-only)* | `(scenes)` | `true` once the editor's bake queue is asked for; returns at once. Refused as `GenerateAsync` is, on unsaved changes to the open scene, and when it names no scene, which leaves an unfinished queue as it was |
| `Lighting.GetBakeQueue` *(dev-only)* | `()` | the queue file as `GenerateScenes` returns it, plus `owner` (the last process to run it), `running` (a live process bakes it now) and `unfinished`; `nil` when there is none, or when the file is unreadable (said in the console) |
| `Lighting.ResumeBakeQueue` *(dev-only)* | `()` | `{ resumed, message, scenes }`: the unfinished queue continued, in this call headless or by the editor's queue in the editor; `resumed` is `false`, with why in `message`, when there is no queue or it is finished |
| `Lighting.BakeOcclusion` *(dev-only)* | `()` | `{ cells, objects, occluders, neverCulled, bytes, tooLarge }` — only Generate Lighting's occlusion step (*Occlusion culling*, below), written beside the scene; the counts `0` when nothing occludes, and `tooLarge` (a boolean) `true`, with `cells` and `bytes` `0` and no PVS kept, when the level is too large to cull |
| `Lighting.GetSettings` *(dev-only)* | `()` | `{ texelsPerUnit, samples, directSamples, bounces, seed, directional, maxLightmapSize, denoiser, backfaceTolerance, samplesPerPass, prioritizeView, diffuseModel, mixedLightingMode, reflectionResolution, reflectionBudgetMB, reflectionMaxProbes }` |
| `Lighting.SetSettings` *(dev-only)* | `({ texelsPerUnit?, samples?, directSamples?, bounces?, seed?, directional?, maxLightmapSize?, denoiser?, backfaceTolerance?, samplesPerPass?, prioritizeView?, diffuseModel?, mixedLightingMode?, reflectionResolution?, reflectionBudgetMB?, reflectionMaxProbes? })` | — |
| `Lighting.Bake` *(dev-only)* | `([probeSpacing], [reflectionRegion])` | `true` if at least one GPU bake ran, `false` if both skipped (no adapter) |
| `Lighting.BakeLightmaps` *(dev-only)* | `([texelsPerUnit], [samples], [bounces], [seed], [directional])` | number of lightmaps written |
| `Lighting.ClearLightmaps` *(dev-only)* | `()` | — |
| `Lighting.GetBakeReport` *(dev-only)* | `(kind)` | the last report the `kind` bake (`"lightmaps"`, `"probes"`, `"reflections"` or `"occlusion"`) wrote beside the open scene, as a table (*Bake reports*, below); `nil` when the scene is unsaved or that bake never wrote one; an unknown kind errors |

### Generate Lighting

- **Settings live with the scene** (Unity's Lighting Settings): texels per unit, samples,
  direct samples, bounces, seed, Max Lightmap Size, the denoiser, Backface Tolerance, the directional mode and
  the progressive bake's samples per pass and Prioritize View are saved in the scene file's
  `lighting_settings` block, so a rebake never asks for them again.
  `Lighting.SetSettings{ samples = 256 }` changes only the fields it names (each clamped
  to the editor's range: `0.25`–`64` texels per unit, `1`–`4096` samples, `0`–`4096`
  direct samples, `1`–`16`
  bounces, `0`–`1` Backface Tolerance, Max Lightmap Size below; a NaN or infinite density takes
  the default `8` (a NaN or infinite tolerance its default `0.9`), and
  a file's out-of-range value is held the same way when the scene loads); the editor's card edits the
  same fields under the button. The reflection probes' defaults sit beside them:
  `reflectionResolution` (default `128`, Unreal's `r.ReflectionCaptureResolution` and the resolution Unity's Reflection Probe Inspector starts at, a power of two in 16–2048) and
  `reflectionBudgetMB` (default `256`, `8`–`2048`) and `reflectionMaxProbes` (default
  `512`, `1`–`4096`, the most probes auto-placement makes), edited in the scene Inspector's
  *Reflection Probes* section (see `Reflection.md`). A scene whose settings are all
  at their defaults leaves the block out of its file. `Lighting.BakeLightmaps` takes the
  same settings for every argument it is not given.
- **The denoiser** (`denoiser`, #1884, Unity's Lightmapping *Denoiser* and *Filter*) cleans the baked
  bounce: `"OIDN"` (the default), `"Filter"` or `"None"`, read in any case; an unknown
  name is an error and changes nothing. The editor's card picks it under the bake knobs.
  See *Denoising* under the lightmaps below for what each does and what OIDN needs.
- **The diffuse model** (`diffuseModel`, #1698) is how a lamp's direct light falls off
  across a surface, scene-wide: `"Lambert"` (the default, the physical `max(N·L, 0)`)
  or `"HalfLambert"` (the retro squared wrap `((N·L + 1) / 2)²`: a quarter of the light
  at the terminator, none only straight behind, and about a third more diffuse
  overall). The name is read in any case; an unknown one is an error, and a
  `SetSettings` call that raises changes nothing. The editor's card edits it under the
  bake knobs, greyed out while a bake runs. It is left out of the file at `Lambert`.
  - **Where it applies:** wherever a lamp lights a surface. Realtime shading, lightmap
    texels, the light probes' baked lamps (and `Probe.BakeAnalytic`'s sun), the surfaces
    a bake's bounce rays hit and those a probe capture renders. So global illumination
    gets brighter under `HalfLambert` too; only the transfer from one surface to the
    next, and the sky, stay Lambert. Live lights follow a change at once, baked light at
    the next bake.
  - **Baked vs Realtime:** one lamp lights the side of a surface facing it the same
    either way. Past the terminator, under `HalfLambert`, they agree only when the live
    lamp casts shadows: a bake always traces a shadow ray, and from a point facing away
    from the lamp that ray hits the surface's own mesh, as a shadow-casting live lamp's
    shadow map does. A live point or spot lamp without shadows (`Light.SetCastShadows`,
    off by default; the sun always casts them) keeps the wrap's back-side light (fading
    from a quarter at the terminator to none straight behind); a baked or shadowed
    one cuts it to dark at N·L = 0, a hard step where the wrap still gives a
    quarter. A dynamic object lit through the probes beside it keeps the back-side
    light: a probe stores the lamp unshadowed at its own position. This step is a known
    limit of the opt-in retro mode; under `Lambert` there is nothing past the
    terminator to lose, and every path agrees.
- **The Mixed Lighting Mode** (`mixedLightingMode`, #1695) is Unity's scene-level
  *Lighting Mode*: what a `Mixed` light bakes. `"BakedIndirect"` (the default, left
  out of the file) bakes its bounce only; `"Shadowmask"` also bakes its static
  occlusion, into a shadowmask page beside each lightmap page
  (`<scene>.lightmaps/lightmap_mask_<page>_<tag>.png`, RGBA8, one channel per masked
  light) and into the light probes, so its static shadows hold past the shadow
  distance (see [`Light`](Light.md), *Shadowmask*). The scene file's `lightmaps` block
  records each masked light's channel (`channels`) and the lights that fell back to
  `Baked` (`fell_back`). The name is read in any case and an unknown one is an
  error; the editor's card edits it beside the diffuse model. A change takes effect
  at the next bake. Whether static casters also draw a masked light's realtime shadow
  is the quality preset's `shadowmask_mode` ([`Graphics`](Graphics.md#quality-presets)).
- **The result is written at once**, as Unity writes its lighting data asset: the
  lightmap pages, the probes' baked SH (`<scene>.lighting.json`), and the scene file's
  lighting fields (probe layout, reflection probes, lightmap set, settings). Only those
  fields of the file are rewritten: unsaved edits elsewhere in the scene **stay unsaved**,
  in the editor and on disk. A scene file loading would rewrite (a hand-edited one,
  say) is left untouched instead: `saved` is `false`, the editor marks the scene unsaved
  whether this call, an MCP call or the Inspector's button ran the bake, and saving the
  scene keeps the lighting. The file's lightmap set names only entities
  the file holds, as a Save's does: a lightmapped mesh that exists only in the unsaved
  edits keeps its lightmap in the scene, and the file gets its entry once the scene is
  saved. A file whose lightmaps name an entity it does not hold drops that entry on load,
  with one warning (a new entity could otherwise take its id and its lightmap), and the
  drop is not an unsaved edit.
- **Refused during Play**, as `Scene.Save` is (#1340): it writes the scene file, and the
  World then holds the play state. The editor's button is greyed out while playing.
- **Needs a saved scene** (the bakes write into folders named after its file,
  `<scene>.lightmaps/` and `<scene>.reflections/`); errors otherwise. The
  probe step needs a GPU/software adapter and skips gracefully without one, as
  `Lighting.Bake` does.
- **Reproducible**: run again on an unchanged scene, it writes the same lightmaps,
  light probes, occlusion and reflection cubes, byte for byte; only the bake reports
  beside them (`<scene>.bake/`, their times) change. A reflection capture reads no
  earlier bake's cubes and sees the scene at time zero (`Reflection.md`, *Bake*).
- **In the editor** the lightmap step runs in the background (#808) with a progress
  bar and **Cancel**, and **progressively** (#1970, *Progressive bake*, below): the
  whole scene sharpens in place pass by pass. A cancelled run writes nothing and keeps
  the old lighting. When
  the lightmaps land they are applied (the probes are captured under them), then the
  probe step runs a step a frame and the console logs one summary line at the end.
  The probe step's placement is planned whole in the frame it starts: about 0.75 s
  in a release build on a Dust2-sized level (10 × 10 rooms of 20 m and a 40 m tower:
  0.41 s for the occupancy grid and floor plan, 0.05 s for the reflection probes,
  0.29 s for 7,638 light probes), so the editor stalls that long once.
- **From a script it logs its progress** (#1667), as Unity's batch-mode bake does: the
  verb blocks its caller until it returns, so a `play` scenario or a `session-mcp` call
  cannot poll, and instead the bake prints a line to **stderr** at each step and every
  5 s within one. An MCP tool call that carries a progress token hears the same
  lines as `notifications/progress` (#1999, `docs/mcp.md`); whether that keeps
  the client's call alive is the client's choice, so prefer `GenerateAsync` there. `Lighting.Generate()` first names its steps (#1897):
  `[Lighting] steps: lightmaps → lightmap pages → light probes → reflection probes → occlusion`.
  Each line then starts with the time since the bake began (#1891), gives an ETA from
  the step's rate over its last 2 minutes (#1890: a texel's cost varies with the lights
  around it, so a rate since the step's start would drift) and, under `Generate()`,
  the steps still to come:
  `[Lighting] 26:02 lightmaps 41% (612/1501 meshes, 3.1M/7.6M texels), ~38 min left (then: lightmap pages, light probes, reflection probes, occlusion)`,
  then a line per lightmap page encoded (`lightmap pages 50% (6/12 pages encoded), ~30 s left`),
  `light probes, bounce 2 of at most 5: …, ~1 min left in this bounce` and
  `reflection probes 19% (3/16 probes), ~4 min left`. At the end (#1891) it prints what
  each step took (`[Lighting] lightmaps took 1 h 4 min (7.6M texels)`, one line per
  step and per light-probe bounce, plus the work between them: `writing lightmaps`,
  `probe placement` and `saving`; every second is in one line, so they add up to the
  total), the 15 lightmap
  meshes whose texels cost the most worker time, summed over every thread
  (`[Lighting]   Hall_Ceiling: 210.4k texels, 312.5 s, 673 texels/s`), and
  `[Lighting] done in 1 h 9 min`.
  The lightmap step's first line and the total also go to the console (so `play`'s
  `console.log` and the MCP reply keep them), beside the probe step's own `Baking …`
  lines; the lines in between, the step plan and the summary go to stderr only. A probe
  step with no probes of its kind prints nothing. `Lighting.Bake()`,
  `Lighting.BakeLightmaps()`, `Probe.Bake()` and `Reflection.Bake()` log the same way,
  without the step plan. A log never changes what is baked: the lightmaps are
  byte-identical with or without one.
- **The probes bake from a snapshot and apply at the end** (Unity's shape, #1602): the
  light and reflection probes are copied when the probe step starts, and their SH and
  cubemaps are applied in one step when it finishes. Adding, removing or moving a probe
  (or changing a reflection probe's resolution, clip planes or shadows) while it runs
  drops the bake with a console line: the probes keep their old lighting and the
  already-applied lightmaps are kept, with the scene marked unsaved; a probe step that
  fails for another reason (a cubemap that cannot be written) marks it unsaved too.
  Saving this one elsewhere drops it the same way; opening another scene drops it with
  nothing applied to the new one.
- **One bake at a time:** `Lighting.Generate()`, `Lighting.BakeOcclusion()`,
  `Lighting.Bake()`, `Lighting.BakeLightmaps()`, `Probe.Bake()` and
  `Reflection.Bake()` error while the editor's Generate Lighting, Bake Occlusion or
  bake queue is running.

### Bake queue

**`Lighting.GenerateScenes(scenes)`** bakes several scenes one after another, unattended:
each one alone, as its own Generate Lighting, and the queue picks up at the first
unfinished scene after a crash, an out-of-memory kill or a reboot. It is not Unity's
`Lightmapping.BakeMultipleScenes`, which bakes scenes *together* as one lighting set.

- **Which scenes.** A list of project-relative paths (as `Scene.Load` takes them), in
  the order given; `"all"`, every `.scene` file in the project sorted by path (the
  `cache/` folder and hidden folders left out), so every run bakes them in one order;
  or `{ all = true, except = { "assets/scenes/kolvik.scene" } }`. A path that names no
  scene is that entry `failed`, and the queue goes on. An ask that names no scene at
  all (an empty list, `except` without `all = true`, or `all` with every scene
  excepted) is an error naming the verb called, raised before anything is written, so
  an unfinished queue on disk keeps its resume point.
- **What a scene's bake is:** its navmesh first (`Navigation.Bake()`), when the scene
  has navigation (it keeps a navmesh, or a `NavMeshAgent` walks it), since the light
  and reflection probes are placed on its floor; then Generate Lighting; then the scene
  saved when the bake could only apply what its file refused (*Generate Lighting*,
  above). Each scene is read from its file beside the open one, so the caller's World,
  and a script running in it, are never replaced; the open scene is read again from its
  file at the end when the queue baked it.
- **Refused** during Play (asked for included), while a bake or a queue runs, and when
  the open scene has unsaved changes: save it first.
- **A failure goes on to the next scene.** A scene whose bake errors is `failed`, with
  why, and the queue continues; the call returns every scene's state, and the console
  ends with a summary line per scene.
- **The queue file**, `cache/bake-queue.json` in the project (gitignored with the rest
  of `cache/`), holds the scenes in order, each `pending`, `running`, `done` or `failed`,
  with its reason (or its bake's summary), how many times it was started and when it
  started and finished (Unix milliseconds). It is rewritten whole, through a temporary
  file and a rename in the same folder, before and after every scene. A file that is
  truncated, not JSON, or names a state the engine does not know is reported in the
  console and read as no queue. `Lighting.GetBakeQueue()` reads it.
- **One queue, one owner.** The process baking it holds `cache/bake-queue.lock`; the OS
  frees the lock when that process ends, a kill or a power cut included. A second
  process (the editor and `export --bake-resume`, two cron lines) is refused, naming
  the holder's process id.
- **Resume.** `Lighting.ResumeBakeQueue()` continues an unfinished queue; with none, or
  a finished one, it does nothing and says so in its return. A scene found `running`
  with the lock free was cut under its bake (the process died): it is baked again from
  its start once, and `failed` if it is found `running` again. A new
  `GenerateScenes` replaces an unfinished queue no process is baking. Resuming a scene
  partway through its lightmaps is not offered: a cut bake writes nothing, so the scene
  is the unit that resumes.
- **In the editor**, `Lighting.GenerateScenesAsync(scenes)` is the queue's version of
  `GenerateAsync`: it writes the queue and returns at once, and the editor opens each
  scene in turn, bakes its navmesh, and runs the Generate Lighting button's job on it,
  then opens the scene that was open at the start again. Poll `Lighting.IsRunning()`,
  which reads `true` from the request to the end, between two scenes included, and
  `Lighting.GetBakeStatus()`, whose message says where the queue is; it ends `done`,
  `failed` when a scene failed or the queue stopped with scenes left, or `cancelled`,
  with the summary. It is refused where
  `GenerateAsync` is, and on unsaved changes. The bake card's Cancel stops the scene in
  hand, which writes nothing as for one bake, and leaves it and the rest `pending`: a
  cancel is no failure and spends none of the scene's starts. Play is refused while
  the queue runs, with the same rule and tooltip as one bake (*A bake and Play*,
  above). An edit made to a scene while the queue bakes it is saved to its file when
  its bake ends, whatever the outcome, and so is a bake only a full save keeps (`saved`
  `false`, above): the scene is saved whole before it is recorded and before the next
  one opens. If that save fails the scene reads `failed` with why, and its lighting
  stays on the open scene, which stops the queue there as unsaved. Unsaved edits to a
  scene the queue did not open (one the user opened instead) stop the queue there,
  with the scene left open and the rest `pending`. "Unsaved" is the editor's mark,
  which only a save clears (*Editor*, *The unsaved mark follows the history*): a
  script's `Scene.Save()` is one, so an edit it saved holds back neither the start nor
  the next scene. The scene inspector's bake card shows the queue's position while it
  runs and, when an unfinished queue is on disk and no process bakes it, how many
  scenes are left with a **Resume** button (`Lighting.ResumeBakeQueue()`), disabled,
  as the verb is refused, while the open scene has unsaved changes.
- **From the command line**, with no window:
  `export --project <dir> --bake all --except assets/scenes/kolvik.scene` (or
  `--bake <scene>...`) and `export --project <dir> --bake-resume`. Each scene's line
  goes to stdout and the bake's progress to stderr; the exit status is `1` when a scene
  ended `failed`, after baking the rest, and `2` when the queue could not start, an ask
  that names no scene included (it prints why and leaves an unfinished queue as it was).
- **After a reboot.** The engine cannot start itself when the machine comes back: the
  queue file is what survives. Starting `export --bake-resume` at login is the OS's
  job, and the engine installs nothing. On Linux, a systemd user unit
  (`~/.config/systemd/user/rusty-bake.service`, then
  `systemctl --user enable rusty-bake`):

  ```ini
  [Unit]
  Description=Resume the rusty bake queue

  [Service]
  Type=oneshot
  ExecStart=/path/to/rusty/target/release/export --project /path/to/game --bake-resume

  [Install]
  WantedBy=default.target
  ```

  On macOS, a launchd agent (`~/Library/LaunchAgents/rusty.bake.plist`, then
  `launchctl load ~/Library/LaunchAgents/rusty.bake.plist`):

  ```xml
  <?xml version="1.0" encoding="UTF-8"?>
  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  <plist version="1.0">
  <dict>
    <key>Label</key><string>rusty.bake</string>
    <key>ProgramArguments</key>
    <array>
      <string>/path/to/rusty/target/release/export</string>
      <string>--project</string><string>/path/to/game</string>
      <string>--bake-resume</string>
    </array>
    <key>RunAtLoad</key><true/>
  </dict>
  </plist>
  ```

### Bake files and the scene save

A scene's bakes sit beside it in files of their own: `<scene>.occlusion` and
`<scene>.navmesh` ([`Navigation`](Navigation.md)). As Unity's lighting data is an asset a
scene save never rewrites, `Scene.Save()` writes one only when this scene baked since it
last read or wrote that path, or holds other bytes than it did then. So when another
process (a headless `session-mcp`, a second editor) re-bakes a scene while it is open,
saving the open one keeps the newer file, and the console says so once (`changed on disk
since this scene read it … the save keeps that newer bake, and the next load uses it`). A
scene that baked since still writes its own (the last write wins, and the warning says it
overwrote the newer file), and a bake that kept no PVS removes the file, newer or not
(the last write wins). A file removed outside the engine is written back when the scene
holds a bake (the console says so). A file refused on load (corrupt, truncated) is
rewritten or removed by the next save while it is still that file; a valid one another
process baked there since is kept, as after a load. One that could not be read at all is
left until the scene bakes. Generate Lighting writes the PVS file only once its PVS is
baked: a bake that stops before (an error, the editor closing) leaves the file as it was.
"Changed" is the file's length and content hash, so a same-length rewrite counts. Writes
are atomic and synced to the disk: a crash or a power cut leaves the old file or the new
one.

The keep-newer rule protects only a file written after this scene's own load of that
path. A save to a path the scene has no record for (File ▸ New, `Scene.New()` or Reset
Scene then a save over a baked scene, a *Save As* over another scene) replaces that path's bake
files with what the scene holds, and removes the ones it does not hold, as Unity drops a
replaced scene's lighting data.

The probes' SH sidecar, `<scene>.lighting.json`, is not under that rule: its entries
are index-aligned to the probe positions the scene document places, so it is written
with the document on every save (removed when no probe is baked). It carries the probe
count and a hash of their positions, and one saved for other probes than the document's
is refused on load with a warning (the probes load unbaked), so SH never lands on the
wrong probe. The lightmap pages and reflection cubemaps are written by their bakes
only, never by a save.

### Light and reflection probes

`Lighting.Bake()` (#246) auto-places light probes and reflection probes from the static
scene, then runs **both** probe bakes (`Probe.Bake` + `Reflection.Bake`) in one call: the
probe half of Generate Lighting, and the orchestration over the `Probe` and `Reflection`
namespaces, which stay available for manual placement. The placement is planned into
a copy: the scene keeps its old probes and their lighting until both bakes finish, and
then the new sets replace them in one step (as Unity's bake applies only when it
completes). A bake that is refused (no saved scene to write cubemaps beside), dropped
(the probes edited midway), fails (a cubemap that cannot be written) or has no GPU
adapter changes nothing.

**Auto-placement** (deterministic — a pure function of the static scene, no RNG/clock,
so the same level always yields the same layout). Both sets are planned from the same
two pictures of the level (#1737):

- the **floor plan**: where actors stand. With a baked **nav surface** it holds only
  **reachable** floor: the **main floor**, always, which is the largest area joined
  by walking that is not an **elevated top** (an area lying over at least half of
  another area at least a quarter its size: a roof over its room, so an indoor level
  whose roof is larger than its room still probes the room, while a small sealed
  nook under the floor does not lift it); plus every area that holds a `NavMeshAgent`
  or `CharacterController` (a sentry on a roof adds the roof, never removes the
  floor); and what off-mesh links (drops, jumps) lead to from those. So wall tops,
  roof tops and crate tops no one reaches get no probe. The rule still misreads a
  main floor over a basement at least a quarter its size as a top; an actor start
  on that floor adds it back.
  Without a nav surface the plan is rasterized from the static colliders' bounds
  (each cell's lowest top) and has no connectivity to read, so every floor in it counts.
  Either way, **a liquid is not floor**: under a static, flat, `Transparent` mesh with
  no solid collider (a pool's water plane), the first floor below its surface is
  dropped when it is at least 0.3 m down, so no probe captures from at or under the
  water. A floor further down (the storey under a rooftop pool) stays, and so does the
  floor under a puddle or grime quad laid just over it. A collider-less glass pane laid flat reads as a liquid
  too; give a skylight a collider.
- an **occupancy grid**: the static solids voxelized at **0.5 m**, one bit a cell: the
  active solid colliders (their triangles; convex shapes filled), and the opaque
  triangles of every static mesh whose entity has no solid collider (their surface;
  so a ceiling built as a render mesh roofs its room). Transparent runs are left out,
  so water and glass are not solid. About 1.6 MB for a Dust2-sized level
  (200 × 200 × 40 m), built in about 2 s in a debug build (3.4 s when every wall is
  also a collider-less mesh); a level past 256 M cells is gridded coarser.

- **Light probes** — after Unity's Adaptive Probe Volumes: a lattice through the air
  over every reachable floor (each storey), from 1 m up to 3 m of headroom, never past
  a ceiling, through a solid between the floor and the probe, or behind a wall from the
  floor cell it was placed over. Its coarse points, at `probeSpacing` (default `4.0`,
  held to at least `1.0`), are always kept; the points between, at half that, only
  near geometry (a horizontal ray meets a wall or a crate within half
  the spacing). A probe closer than 0.3 m to geometry is pushed out of it, and one still
  inside a solid is dropped. The layout has no regular grid, so objects blend the
  four nearest probes (`Probe.md`), found through a spatial index: about 1.5 µs a
  sample in a release build over a Dust2-sized set of 7,680 probes, against 12 µs for
  a scan of all of them. A moving Light Probe Proxy Volume samples each of its
  points, so a 32³ one costs about 50 ms on every frame it moves. With no static solid at all, a regular grid over
  the nav surface's walkable extent is placed instead.
- **Reflection probes** — **one per room** (#1584). The floor plan
  is cut into rooms where it narrows
  below **2.5 m** (a doorway, an arch, a corridor mouth) or steps by more than 0.5 m.
  Each room's probe captures from **eye height (1.6 m) over the room's middle**,
  standing on open floor at least 0.5 m clear of geometry where it can, with its box
  **fitted to the room's walls, floor and ceiling** (lowered to the first solid
  straight above the capture point). A room where most of its probes' upward rays
  (straight up and a ring at 30°) reach the sky is **open air**: it gets **one probe
  without box projection**, not a box under an invented ceiling, at importance `0`:
  its box spans the room's whole footprint, so the room probes inside it (importance
  `1`) draw over it. It captures from the clear point nearest the room's middle that
  is itself open to the sky, so a yard round a house never captures from the house's
  doorway.
  A room wider than its region is split into probes no wider than it: the region is
  `reflectionRegion` (default `12.0`, at least `2.0`: a smaller one is
  an error) for an empty room, narrowed by its **clutter** (the solid fraction of the
  air between 0.4 m and 1.8 m over its floor, column by column from each one's own
  floor, the blocked patches of up to 16 m² it encloses counted: a room a quarter
  full of crates takes probes a quarter as wide, while the solid core a corridor wraps
  is architecture, not clutter). Open floor too narrow to hold a room
  (a level of corridors narrower than a doorway, a lone ledge) is an area of its own and
  gets its probes the same way, unless it is under 9 m² (the top of a jump box or a
  crate): that joins the room around it. `reflectionMaxProbes` is a ceiling the manual
  probes count against: a layout over it is re-tiled wider, with a console warning,
  until it fits or every room is one probe, and past that the rooms with the largest
  boxes keep theirs. With the manual probes already at it, none is placed. Without a nav
  surface the floor plan is rasterized at 0.5 m, or coarser (with a warning) when the
  level would take more than 4 million cells. Triggers are not walls.
  A floor plan finds rooms on one floor per cell, so a room stacked under another (a
  tunnel under a catwalk) shares the lower floor's probes.

**Manual vs. placed** (#1737): every probe is either **manual** (`Probe.Add`,
`Probe.FillGrid`, `Reflection.Add`, the Inspector's *Add probe*, or marked with
`Probe.SetManual` / `Reflection.Set(i, { manual = true })` / the Inspector's *Manual*
box) or **placed** by this bake. A hand edit makes a probe manual: `Probe.Move`,
`Reflection.Move` / `SetBox` / `Set`, an Inspector field (except `Set{ manual = false }`
and unticking *Manual*, which say otherwise), so a tuned box survives the next bake.
Each bake **replaces the placed probes and keeps every manual one as it is** (position,
box, settings), and plans around them: a placed reflection probe whose box a manual
probe's volume (its turned box or sphere, not its bounds) covers by at least half is left out (that room is the manual probe's), and
no placed light probe lands within half a step of a manual one. A manual `FillGrid`
keeps its trilinear grid through any number of bakes. So a level can mix the two and still be regenerated after its static
geometry moves. A saved probe without the flag loads as placed. `Probe.Clear()` /
`Reflection.Clear()` still remove both kinds.

Like the individual bakes, it is **dev-only** and needs a GPU/software adapter; with none
the placement still runs but the bakes skip gracefully (returns `false`, never errors —
except reflections need a saved scene path: their cubemaps go in its own
`<scene file>.reflections/` folder, and a reflection bake that runs removes that folder's
cubes past the probe count, all of them when the scene has no reflection probes). Save the scene
afterward to persist the baked SH (`<scene>.lighting.json`) and the cubemap paths, or use
`Lighting.Generate()`, which writes them.

#### Probes per loaded scene

Each scene loaded with `Scene.Load(path, "Additive")` ([`Scene`](Scene.md)) brings
its own light probes and reflection probes, as in Unity: its `<scene>.lighting.json`
SH is checked against its own probes alone, so no scene's set is renumbered when
another arrives or leaves. The active scene's probes are the ones the `Probe` and
`Reflection` namespaces, the Inspector, a bake and a save see; the others wait with
their scene and come back when it is made active. An unload takes its scene's probes
with it.

- **Light probes are sampled as one field.** A point inside a set's grid
  (`Probe.FillGrid`) reads that grid, the first loaded scene's when two overlap; any
  other point blends the nearest probes of every loaded scene together, so an object
  standing between two scenes is lit by both. A point at a border changes light when
  its neighbour unloads (a scene unloads at the streaming range, far from the camera).
- **A change re-samples only what it can reach.** When a scene's set arrives, leaves
  or is edited or rebaked, the renderer re-samples the objects and Light Probe Proxy
  Volumes near it, and no other. A point blends its nearest probes, and the distance to
  the farthest of them grows by at most as much as the point moves, and only grows when
  probes leave: so with the changed set left out, a point (a proxy volume: its box's
  centre, its reach widened by twice its radius) whose reach falls short of the changed
  set's box, before and after the change, reads bit for bit what it read.
- **Reflection probes**: each scene's cubes stay in its own `<scene>.reflections/`
  folder, and each scene's `reflectionMaxProbes` caps its own placement. The cubes
  resident on the GPU are picked nearest the camera first across every loaded scene,
  under the active scene's `reflectionBudgetMB`. Only the active scene's fallback
  probe is one (`Reflection.SetFallback`, Unity's active scene's environment reflection
  wins); another scene's is an ordinary probe while its scene is not active.
- **Shadowmask occlusion is read from the `"Single"` scene's probes alone.** An
  additive scene's probes keep the occlusion its own bake wrote, but its channels are
  its own `Mixed` lights', and the World's shadowmask channels are the `"Single"`
  scene's (an additive load applies no lightmaps), so its probes read unoccluded:
  a dynamic object among them takes the `Mixed` lights unshadowed by the probes.
- **Placement** plans for the active scene: its own static solids span the floor plan
  and the light probes' fallback grid, and every loaded scene's solids stand on it,
  roof it and block it, so a neighbour's wall still occludes.

### Lightmaps

`Lighting.BakeLightmaps()` (#438) bakes per-texel lighting for **static geometry**: bounce
light, the sky, emissive surfaces, and the direct light of `Baked` lights (see the light
**Mode** in [`Light`](Light.md)). It is the lightmap half of Generate Lighting; the
editor's button runs the same bake as its first step, in the background (#808) and
progressively (#1970): the window stays live, a progress bar counts texels done (over
every pass) and then the pages it encodes ("Encoding page k of n"), and **Cancel** stops
the bake, during the trace or the encode, and keeps the previous lightmaps. The result
is applied when the bake finishes, byte-identical to the script verb's; a bake whose
scene was swapped out meanwhile is dropped. The script verb itself stays synchronous
and one-shot, as does `Lighting.Generate()`. An argument left out takes the scene's
lighting settings (above), whose defaults are the ones listed here.

- **Progressive bake** (#1970, Unity's Progressive Lightmapper; the editor's Generate
  Lighting). The trace runs in passes over **every texel of the scene**, not mesh by
  mesh: each pass adds `samplesPerPass` paths (default `8`, `1`–`4096`; eight passes at
  the default 64 samples) to every texel, up to `samples`. After each pass but the last
  the live scene shows the lightmaps as they stand: the paths so far averaged, buried
  texels filled, the bounce cleaned by the denoiser, packed into pages. A writer
  thread beside the bake stores them as `lightmap_preview_<bake>_*` pages in the scene's
  lightmap folder (uncompressed PNGs of one level, no BC encode, while the trace goes
  on; a page a pass left unchanged is not written again); a pass still unshown when
  the next is in is skipped and its pages removed,
  and while the writer is busy the bake builds no preview at all, so a pass that
  would only be dropped costs no denoise. The card and the log say
  the passes done, the samples so far and the time left at the pace since the first
  pass (`[LightmapBake] lightmaps pass 3/8 (24/64 samples), ~3 min 5 s left`).
  **The finished bake's bytes are the one-shot bake's** at the same settings and seed:
  each texel keeps its own seeded stream and its running sums between passes, so the
  pass size never shows. Only a finished bake is written: a save during the bake
  writes the scene's previous lightmaps, never a preview, and **Cancel** puts them
  back and removes the preview pages. Edits during the bake reach those hidden
  lightmaps: a deleted entity leaves them and the preview, and
  `Lighting.ClearLightmaps()` clears them and the preview. The finished bake still
  keeps the deleted entity's new entry, so an Undo of the delete brings it back lit by
  this bake; a save writes no entry for an entity the scene no longer holds. Each shown pass
  costs a display denoise and a preview write, so a smaller `samplesPerPass`
  refreshes more often and bakes a little longer. Until the first pass is done the
  card says `first pass`. The texels' state between passes
  is held once, updated in place.
  Rebaking on edit (Unity's *Auto Generate*) is not built yet.
- **Prioritize View** (`prioritizeView`, default `true`, Unity's setting of that name).
  The texels the **Scene view's** camera sees converge first: in its frustum, facing
  it (for an orthographic view, its view direction), with nothing static the camera
  draws in between (one ray per texel, before the first pass; a `"ShadowsOnly"` mesh
  hides nothing). The whole scene gets its first pass, so it shows at once; then the
  seen texels get all their other passes, each shown, then the rest get theirs. So a
  bake whose view sees part of the scene makes more steps than passes (`pass 3/15
  (8/64 samples, 24 in view)`), and the same paths in all. Only when each texel is
  traced changes, never the bytes; off, or with a view that sees all or none of the
  scene, every pass covers every texel.

- **Who gets one.** Every active, **static**, opaque mesh with a **second UV map** (the
  lightmap UV: glTF `TEXCOORD_1`, or one generated at import by the model's **Generate
  Lightmap UVs** setting, see [`Assets`](Assets.md#generate-lightmap-uvs)). Every built-in
  primitive carries one (the Sphere, Cylinder and Capsule through that same unwrap); an imported
  mesh without one keeps probe / ambient lighting, and the bake logs once how many static
  meshes were left out that way: *"N static meshes have no lightmap UV — enable Generate
  Lightmap UVs"*.
  Every static mesh still shades and bounces light onto the others, as its Mesh
  `cast_shadows` allows ([`Light`](Light.md#cast-shadows), one rule for every bake):
  an `"Off"` mesh blocks no light but still bounces, emits and is lightmapped; a
  `"ShadowsOnly"` one blocks light but bounces, emits and receives nothing. A surface
  bounces its material's colour as one average per material: the base colour times its
  base-colour map's average, darkened by its occlusion map's average at its
  `OcclusionStrength` ([`Material`](Material.md)), so a recess's shade bakes the same
  bounce whether the albedo or the occlusion map carries it. A mesh at
  Scale In Lightmap `0` (below) gets no lightmap and reads the light probes. Dynamic objects keep
  the light probes; bake those with `Lighting.Bake()` so both read the same lights.
- **What it bakes.** A deterministic CPU path tracer: `samples` (default `64`, enough for OIDN, below) paths
  per texel, `bounces` (default `5`, where Unity starts at `2`, so a level lit only by its bake reaches rooms the sun never does) surfaces deep, seeded by `seed` (default `0`) — the
  same scene and seed always write byte-identical lightmaps, however many cores ran it
  (on any machine under the `"Filter"` denoiser; on one machine under OIDN).
  Each surface a path lands on aims one shadow ray at one source picked from the lamps
  in range and the emissive surfaces near it, by how much each could light it (#1883,
  Unity's Progressive Lightmapper does the same): a level with 150 lamps bakes about as
  fast as one with a few, and a small bright emissive panel bakes smooth instead of
  speckled. Past the second bounce a dim path may end early (Russian roulette), which
  changes only the noise. What the bake converges to changes in one way: a strong
  emitter (emission past about 10) lights its surroundings brighter than before, since
  its light now arrives through those aimed samples rather than through bounce paths
  the per-path clamp (Blender's *Clamp Indirect*) cut down. Each path is still clamped,
  the texel's own sample of an emitter included, so the pool right beside a very
  bright emitter still bakes under its true value.
  `texelsPerUnit` (default `8`) is the lightmap resolution per world unit, times each
  mesh's **Scale In Lightmap** (below). A mesh's lightmap follows its surface area at that
  density, from 4 texels square up to the largest map one atlas page holds with its
  ring: `maxLightmapSize − 8` (1016 at the default 1024, #1704). A map above half the page
  leaves the rest of it to smaller maps. The bounce is then cleaned by the denoiser
  (*Denoising*, below); baked direct light never is, so a point, spot or directional light's shadows stay sharp; an Area
  light's (#1674) are soft, its rectangle integrated over `directSamples` or more jittered
  points per texel. `directSamples` (#2355, Unity's *Direct Samples* beside `samples`,
  its *Indirect Samples*) defaults to `0`, which follows `samples`. Its points round up
  to a square (`n` points take a `⌈√n⌉ × ⌈√n⌉` set), and how it costs depends on which
  side of 50 it sits. **From 50 up** (a set of 64 points or more) each panel is first
  probed with 20 shadow rays: one to each corner (aimed just inside it, so a recess's
  walls meeting its edge don't block them) and one to a point picked in each of 4 × 4
  blocks of the full set. Mixed (a penumbra), the rest of the set casts its rays, the
  probe's points counting among them, so a penumbra costs the full set plus the four
  corners. All seen, the rest of the set is taken as unshadowed, and 4 more of its
  points, one in each quarter of the panel, cast their rays to confirm it (24 rays a
  panel); all hidden, it is taken as giving nothing, and 32 more points, two in each
  block, spread over rows and columns its other points left, confirm that (52 rays).
  Those two costs hold whatever `directSamples` is, so above 50 the setting only prices
  penumbrae. **Under 50** there is no probe and every panel casts every point of its
  set, so `49` costs more than `64` on a floor that is mostly lit or mostly shadowed,
  and only about `25` or under is cheaper than the probe there. Lowering it trades
  penumbra quality for penumbra speed; it is not a general speed knob.
  The first confirming ray that contradicts the verdict drops it, and the texel casts
  the rest of the set from fresh points, keeping the 16 block points the probe already
  cast: the full set's estimate with those 16 kept, never a weighted mix, so a wrong
  verdict that a check finds costs rays, not speckle. Since those 16 all agreed with
  the dropped verdict, such a texel reads a shade lighter under a thin occluder (or
  darker beside a thin opening) than a plain full set would, by about their share of
  the set. A panel partly below the receiver's horizon is judged, and confirmed, by
  the points whose light reaches the receiver only; the rest cast nothing. What it can
  still lose is a shadow or a sliver of light from an occluder or an opening too thin
  for those two dozen or so points to find (a pipe under a panel, a narrow slit), and
  only on the texels where all of them missed it; such a texel is off by at most that
  occluder's or opening's share of the one panel's light. Measured straight under a
  1.2 × 0.6 m panel at 256 points, a rod across it 1/8 of its width keeps its shadow on
  95% of texels, and a slit 1/16 of its width lets its light through on 98%.
  A texel buried inside other geometry (floor under a crate) is filled from its neighbours rather than baked black (*Backface
  Tolerance*, below). No GPU is needed; it runs headless.
- **Backface Tolerance** (`backfaceTolerance`, #1949, Unity's setting of that name,
  default `0.9`, `0`–`1`). Each texel shoots 32 probe rays of its own; it is
  *valid* when at least that share of them reach a front face or escape. A texel
  below it sits inside geometry (a floor texel under a crate, one inside a wall): its
  light is dropped and it is filled from the valid texels of **its own chart** (the
  UV island it lies on), never from a chart packed beside it in the map, so a texel
  at a floor's edge never takes the dark underside next to it. The probes are a fixed,
  seeded budget apart from `samples`, so a texel is valid or not the same way at 16
  samples and at 4096. `0` keeps every texel; `1` drops any that sees a back face.
  **One-sided geometry counts as inside:** a floor under a single quad roof sees
  only the roof's back, so at `0.9` its texels are dropped and filled from the
  floor's lit part beyond the roof. The bake has no *Double Sided GI* yet, so close
  such geometry (a box, not a plane) or lower the tolerance for that scene. A
  chart with no valid texel at all (a box's face resting on a floor or against a
  neighbour, which is ordinary content) is filled from the charts beside it, as
  before, and the bake log says how many in one info line, never a warning.
- **Denoising** (#1884). Only the bounce (indirect light, and a directional bake's
  directions) is cleaned, chart by chart; the direct light stays as traced.
  - `"OIDN"`: Intel **Open Image Denoise 2**'s `RTLightmap` filter, a network trained
    on lightmaps, then the `"Filter"` Gaussian below, as Unity runs its denoiser and
    then its filter (OIDN alone leaves a faint mottling on flat walls at low
    samples, which the Gaussian takes out). At `64` samples the pair comes as close
    to a converged bake as the Gaussian alone did at `128` (within a point of mean
    relative error, measured on a primitive interior, #1884), in about 60% of the
    time, so it is the default and `64` the default `samples`. OIDN is a native library the machine must have, the way
    Blender is for a `.blend`: **macOS** `brew install open-image-denoise`, **Arch**
    `pacman -S openimagedenoise`, elsewhere the release from
    `github.com/RenderKit/oidn/releases`, unpacked anywhere with the `RUSTY_OIDN`
    environment variable naming its folder (or the library file). rusty opens it when
    a bake asks, never at build time, so a machine without it still builds, runs and
    bakes: the bake warns once, naming what to install, and only filters, which at
    `64` samples is blurrier and blotchier (raise `samples`, or pick `"Filter"` to
    silence it). Installed mid-session, the next bake finds it; no restart. Each bake
    worker denoises its own lightmaps on an OIDN device of one thread, beside the
    trace. Its output may differ in the last bits between CPUs (its SIMD path),
    never between runs on one machine.
  - `"Filter"`: an edge-aware Gaussian, 3 texels wide (Unity's indirect filter), that
    never crosses a crease. Byte-identical on every machine.
  - `"None"`: the raw trace, grain and all.
  A logged bake (`Lighting.BakeLightmaps()`, `Lighting.Generate()`) says OIDN's
  worker time and how many lightmaps it cleaned, in a line of its own in the
  summary: it runs inside the lightmaps step.
- **Memory.** A script's bake (one-shot) peaks at what it keeps plus what is in
  flight, never every texel's working data at once. It keeps each finished lightmap
  as the texels its UV islands cover, not its whole square: 16 bytes a covered texel,
  16 more for a directional bake and 16 more for a shadowmask one, so a mesh whose
  islands fill a tenth of its square keeps a tenth of it. In flight are the meshes
  its workers are tracing (about 112 bytes a covered texel each, one mesh per core at
  most) and the ones being assembled (about 256 a texel of their squares; a map
  past 512² is assembled one at a time, so two large maps finishing together wait
  on each other rather than both being held; smaller ones are assembled side by
  side, and the estimate below counts each at its square); a mesh's
  texels are rasterized only when the bake reaches it. OIDN cleans each lightmap
  with at most 512 MB of scratch (a larger one is cleaned in overlapping tiles;
  measured on 1016² maps, byte for byte what one pass gives; a larger map, cut
  into more tiles, is expected to give the same picture without a seam, which
  nothing has checked byte for byte), and the lightmaps
  being cleaned at once share 2 GB, so its share does not grow with the cores.
  Then the pages are made one at a time: each is composed (its lightmaps' squares
  filled again, which is when a map's edge is dilated and the rest of its square
  filled), encoded and written to a scratch folder in the project's `cache/`
  (`cache/lightmap-spill/`, gitignored and never exported; on the project's disk,
  so the final write only links the pages into `<scene>.lightmaps/`; the project
  is the nearest folder above the scene holding a `project.rusty` or an `assets/`
  folder, and only a scene in no project at all spills into the system's temp
  folder, which may be memory, where the page stage is not bounded), and a
  lightmap is let go once its page is composed; so the page stage holds one page
  (about 128 bytes a page texel) and one map's square at a time. The scratch
  folder is removed when the bake ends, written, cancelled or failed. One a bake
  killed by a quit or a power cut left is removed by the next bake of any scene
  of the project: each bake holds a file lock on its own folder, which the system
  drops with the process, so a folder whose lock is free is a dead bake's. The editor's bake (progressive) also keeps every
  texel's sums between passes, about 116 bytes a covered texel. So a bake's memory
  grows with `texelsPerUnit`² and `maxLightmapSize`² (the largest map and the
  page), and with the scene only by its lightmaps' covered texels. Before it
  starts, a bake estimates its peak from those (erring high) and warns, naming
  what holds the peak (finished maps, in flight, OIDN, pages) and the settings to
  lower, when it passes the memory the machine has free (Linux `MemAvailable`;
  macOS free and inactive pages); it is refused with the same message when it
  passes all the machine has. On Linux both are held to the process's memory
  cgroup (v2) when one caps it (`systemd-run -p MemoryMax=…`, a container's
  limit): the cap is the total, and what is free is the cap less what the cgroup
  uses (its inactive file cache counted free). Its last console line and its
  report (`peakResidentMB`, below) say the process's peak resident memory, which
  in a headless bake is the bake's. On macOS that is the peak physical footprint,
  which also counts large blocks the allocator keeps cached after the bake frees
  them (dirty, so Activity Monitor's Memory column counts them too). It is
  libmalloc's cache of freed large blocks, not memory the bake holds: on a test
  bake it read about 70 MB above the cache-off figure at 8 meshes and about
  96 MB at 32, far below the 8 MB a mesh it once took. Two readings do not show
  whether it levels off or keeps growing by about 1 MB a mesh, so the figure
  reads at least that much above what the bake held. On Linux (glibc), a bake also pins
  the allocator's mmap threshold at its default (128 KiB), so every large block
  the bake frees goes back to the system at once; glibc has no way back to its
  adaptive threshold, so this lasts for the rest of the process (an editor
  session after its first bake), at the cost of a page fault per large
  allocation.
- **Max Lightmap Size** (`maxLightmapSize`, Unity's setting; the editor's
  **"Max Lightmap Size"** under the Generate Lighting button). The atlas page's edge in
  texels, a power of two from `32` to `4096`, default `1024`; any other value is held to
  the next power of two in that range, when set and when a scene file is loaded. It caps
  both the page and, through it, the largest lightmap a mesh can get. A bake that would
  take more than **256 pages** (the layers one GPU texture array holds) is refused before
  it encodes, with a message naming the page count: raise Max Lightmap Size, or lower
  `texelsPerUnit` or Scale In Lightmap. The scene keeps its previous lightmaps. A scene
  file that already holds more pages than the GPU takes loads unlit by lightmaps, with a
  warning, until it is rebaked.
- **Scale In Lightmap** (Unity's *MeshRenderer* setting, #1704): a per-mesh multiplier
  on `texelsPerUnit`, the Mesh component's `scale_in_lightmap` field (default `1`, `0`–`64`),
  written with `Component.Set(id, "Mesh", { scale_in_lightmap = 2 })`
  ([`Component`](Component.md)), read with `Component.Get`, shown in the Inspector's Mesh
  card and saved with the scene and its prefabs (a file without it reads `1`). A hero
  prop takes more texels, clutter fewer, without changing the scene's density. At `0`
  the mesh gets **no lightmap** and reads the **light probes** at runtime, as in Unity,
  while still blocking and bouncing light in the bake as its `cast_shadows` says. A
  value read from a file is held to the same range when the scene or prefab loads.
- **Receive Global Illumination** (Unity's *MeshRenderer* setting): where a static mesh
  takes its baked indirect light from, the Mesh component's `receive_gi` field,
  `"Lightmaps"` (the default, and what a file without it reads) or `"LightProbes"`,
  written with `Component.Set(id, "Mesh", { receive_gi = "LightProbes" })`, read with
  `Component.Get` (always written), shown in the Inspector's Mesh card and saved with the
  scene and its prefabs. A `LightProbes` mesh is lit as a mesh at Scale In Lightmap `0`
  is: no lightmap texels, its ambient from the light probes of every loaded scene around
  it, while it still blocks and bounces light in the bake. It keeps its Scale In Lightmap
  for when it is set back to `Lightmaps`. Use it for a piece baked without lightmaps, or
  for clutter too small to be worth texels.
- **Directional** (#810, Unity's *Directional Mode*). By default the bake also stores,
  per texel, the **dominant direction** its light arrives from and how strongly it leans
  that way (its *directionality*: 1 when it all comes from one side, 0 when it comes
  evenly from everywhere). The bounce's directions are smoothed by the same filter as its
  colour. At runtime a lightmapped surface reshapes its baked light by how its
  **normal-mapped** normal faces that direction, so bumps, tiles and panel seams read
  under baked light as they do under realtime light (a surface without a normal map
  keeps its baked diffuse exactly as a non-directional bake gives it). Where `Baked`
  lights supply a quarter or more of a texel's light the direction is theirs (eased
  toward the mean of all its light below that), so there a normal map reshapes the
  texel's bounce by the lamp too. Each texel also stores its **lamp share**, how much
  of its light is `Baked` lights' direct light from one way, which gives a glossy
  surface one specular highlight from those lights ([`Light`](Light.md), #1695) and
  none from sky or bounce; a non-directional bake has none. `directional = false` (or the editor's
  **"Directional lightmaps"** checkbox under the Generate Lighting button) skips the direction pages
  to save their memory; the colour lightmaps are byte-identical either way.
- **At runtime** a lightmapped surface takes its ambient term from the lightmap instead
  of probes or the flat sky gradient, and skips the realtime direct light of `Baked`
  lights (it is in the map). `Mixed` lights stay realtime on it; their bounce is baked.
  Lightmapped copies of one prop still draw as one instanced call.
- **Where the files go.** The lightmaps are packed into square **atlas pages** (the
  smallest power of two that holds them all, up to Max Lightmap Size, then as many pages
  of that size as it takes; a gap in a page is filled before another opens, #1694). Each page is a
  `lightmap_<page>_<tag>.ktx2` in `<scene file>.lightmaps/` (old pages there are removed
  first): BC6H HDR light, a byte a texel, with two mips, which the renderer picks between
  by distance; each lightmap sits in a 4-texel ring of its own edge light, in a cell
  aligned to 16 texels, so no compressed block and no texel of any of the three levels
  ever mixes two lightmaps. An RGBM-encoded PNG of the same
  name sits beside it: what a GPU without BC compression loads, and a picture an agent
  can open. A directional bake writes a `lightmap_dir_<page>_<tag>.ktx2` beside each
  page, packed exactly like its colour page: RGB the direction scaled by its
  directionality, each component `x` stored as the byte `128 + 127·x` (so 0 is
  exact), and A the lamp share (0–1). It is plain RGBA8 (`R8G8B8A8_UNORM`, four bytes
  a texel) with the bake's two mips, uploaded as it is stored, never block-compressed:
  BC7 put a lamp's direction up to 5° off a texel, which a polished surface's baked
  highlight showed as a sawtooth edge (#1996). The texture importer leaves that folder alone
  (`Assets.md`). Encoding takes time after the trace, most of it the irradiance page's
  BC6H and its PNG: a 2048² page pair about 3.3 s on four cores (2.9 s without its
  direction page), a 4096² pair about 14 s (11 s); a level whose biggest meshes reach
  the default 1016 cap fills about three 4096² pairs, so about 40 s (#1704). A 4096²
  page pair takes about 105 MiB of GPU memory (21 MiB without its direction page), and
  direction pages take four times what BC7 did, so a dense directional bake can reach
  memory: a Dust2-sized level's direction pages take about 47–74 MiB at 8 texels a unit
  but 0.75–1.2 GiB at 32. Each page array (colour, direction, shadowmask) may take at
  most **1 GiB** on the GPU, every level of every page (`MAX_ARRAY_BYTES`: wgpu names no
  texture-memory limit, so it is a constant); past it the load is refused with a
  warning naming the pages and the scene is left unlit by lightmaps, so rebake with fewer
  texels a unit. 8 texels a unit already keeps the baked highlight sharp. The editor's Generate Lighting
  encodes the pages and compresses their PNGs on its worker and only writes the finished
  files on the frame, so the window stays live. The scene stores the page list and, per entity, its page and
  scale/offset (Unity's lightmap index + scale/offset), and the lights the bake took
  in (every active `Mixed` and `Baked` light above intensity 0, Unity's
  `Light.bakingOutput`), which
  SSIL leaves out because their bounce is baked (`Graphics.md`, *Screen-space
  indirect lighting*), so **save the scene** after
  `Lighting.BakeLightmaps()` to keep them (`Lighting.Generate()` writes them itself). Needs a saved scene path (errors otherwise). `Lighting.ClearLightmaps()`
  drops the references (the files stay until the next bake).
- **Moving or editing static geometry** after a bake leaves its lightmap stale; rebake.

**Blender export.** The lightmap UV is the mesh's **second UV map**: add one in *Object Data
Properties → UV Maps*, unwrap it with no overlapping islands (*Lightmap Pack*, or *Smart
UV Project* with some island margin), keep the first map for textures, and export glTF 2.0
with *UVs* enabled — Blender writes the second map as `TEXCOORD_1`. Or skip all that and
tick the model's **Generate Lightmap UVs** (Unity's checkbox): rusty unwraps one at import.

### Occlusion culling

Unity's *Occlusion Culling* (#1714, part 2): Generate Lighting bakes, from the static
scene, which static meshes each cell of the level may see, and a camera with
`use_occlusion_culling` on (the default; `Camera.SetUseOcclusionCulling`, the Camera
card's *Occlusion Culling* box) skips the static meshes its cell cannot see. It changes
no pixel: a mesh it skips is one no ray from the camera's cell reaches.

- **What takes part.** rusty has one static flag, so a static mesh is both of Unity's
  `StaticEditorFlags`: an opaque one (no surface shader) is an **occluder and an
  occludee**; a Transparent, Cutout or custom-shader one, one in a `LODGroup`, one
  with a max draw distance or min screen size, and a light's gizmo are **occludees
  only** (what draws of them changes, or a view passes through them). Dynamic,
  skinned, shadows-only and inactive meshes take no part and are always drawn.
- **Cells.** A uniform grid of 2 m cubes over the bounds of the static meshes it can
  cull, one cell of margin round them, coarser cubes past 16384 cells sampled (or 65536
  in all), up to 4 m (a Dust2-sized level, 200 × 200 m on two storeys, takes 3.9 m when
  every cell is sampled). A level that needs larger cells is **not occlusion-culled**:
  Generate Lighting's console line says so, and bakes no PVS. A camera outside every
  cell culls nothing.
- **Which cells are sampled** (#2328, as Unreal places its precomputed-visibility
  cells): only those a camera can be in. A cell is *standable* when it reaches the
  air within the **play height** over a walkable floor, the floor reaching out to
  the walls by the navmesh's agent radius (the walkable surface stops short of them
  by it), so a player against a wall stands in a sampled cell. The play height comes
  from the scene's navmesh settings: `agentHeight` plus the larger of `jumpHeight`
  and 1 m (`JUMP_ALLOWANCE`), never under 3 m (`MIN_PLAY_HEIGHT`: a navmesh baked
  for a small agent still has a camera riding the player, or over their shoulder),
  3 m with the defaults. The cell holding a shadow-casting point, spot or area light
  is sampled too, since its shadows cull by that cell's row; so is the cell of every
  `Camera` in the scene, active or not (a fixed, overhead or cutscene camera above
  the play height keeps its PVS; one moved within its cell keeps the bake, to
  another cell re-bakes it); and so is the ring of cells round each, which the
  dilation (below) reads. The floor is the scene's navmesh, every walkable span,
  while it is baked and current for the scene (the editor keeps it so as you edit;
  one a scene edit has outdated is not read). A headless session's edit frame does
  not sync the navmesh as the editor does: there call `Navigation.Bake()` before
  `Lighting.Generate()` to sample only where a camera can be. With
  none, every cell is sampled, as before this existed, and the console line says
  `no fresh navmesh: every cell sampled; bake the navmesh to sample only where a
  camera can be`. No floor is guessed from the colliders: a guess that missed a
  storey or a building's inside would cull less there than sampling every cell. A
  navmesh that marks no cell of the grid falls back to every cell too, with a
  console warning, and its console line says `the navmesh marks no cell where a
  camera can be: every cell sampled; re-bake the navmesh over the level`; one that covers a small share of the level is still used,
  its share in the bake report's `floorCoverage`, so a partial navmesh shows. Every other cell (the air under a high roof, over a courtyard, under the
  ground floor) **sees every object**: a camera there, a spectator or a camera a
  script moves above the play height, is culled by nothing from the PVS, as outside the
  grid, and the GPU's Hi-Z test still culls. A standable cell keeps exactly what it
  would with every cell sampled. A light a script moves at runtime into an unsampled
  cell culls no shadow caster by the PVS there. Unity's authored Occlusion Areas, the
  developer's override of the play height, are not built yet. Each cell keeps what any
  point in it may see: rays from seeded points to points on each mesh. A cell larger
  than 2 m is sampled as 2 m cubes, six points each, so the eye's density is the same
  at every cell size. The points on a mesh are its extremes and a lattice laid on its
  triangles at most 1 m apart, kept at one per half-metre cube of space, so their
  count follows the mesh's surface area, not its triangles (a 2000-triangle prop gets
  about the points a box of its size does), and a long wall seen through a doorway is
  aimed at wherever the doorway shows it. A point on a one-sided triangle that faces
  away from a sampled cube is skipped. Most rays are never cast (#1714, part 3): an
  occluder face that hides a group of meshes, a mesh, or some of its points from a
  cube or from one of its points proves their rays stopped, and the faces on one
  plane that draw the same side count as one (Unity's occluder fusion, for coplanar
  faces): a floor of tiles, a wall built of pieces with its doorways left open. A
  seam fuses where it falls on the quarter-metre grid of world units, as a modular
  level's do; a slit between two faces is seen through however thin, down to float
  noise, and faces fuse only at one depth (to float noise), never a millimetre
  apart. The faces' triangles fuse whatever their order or triangulation (#1981):
  edges two faces share corner for corner join them. So a wall tessellated into
  triangles that share their vertices, as an exporter writes one mesh (a shared
  corner the same position, bit for bit, once in world space), hides what a box
  does, short of its edges: there it fuses to the quarter-metre texel, where a
  box's whole face is proved exactly. An edge welded only to float noise, or a
  T-junction, is left a seam, which fuses less and is never wrong.
  Each cube tries first the faces that last stopped one of its rays.
  These proofs are exact, so they change how fast a cell bakes, not what it keeps.
  Every cell
  then also takes its 26 neighbours' sets, and cells that see the same set share one
  row. The bake is a pure function of the scene and a fixed seed: the same bytes at
  any thread count.
- **What it can miss, and what it never culls.** A gap that shows less than about a
  metre of a mesh may hide it (points are sampled, not every ray), at every cell size
  the grid allows. A mesh too large to sample (over 4096 points, about 4000 m² of
  surface: both sides of a 600 m wall 3 m high) or with a vertex or a transform that is
  not finite is never culled: it is kept in sight of every cell, lays no cell (a
  ground slab never coarsens the grid), and Generate Lighting's console line and
  `occlusionNeverCulled` count it.
- **Cost** (release build, 4 threads, primitive levels of 10 m rooms on two
  storeys, a doorway in every wall, two crates and a 1150-triangle sphere in each):
  the bench at Kolvik's size (5 × 5 rooms, 397 static meshes, 4704 cells of 2 m) bakes
  in about 4–5 s into 277 KB; a Dust2-sized level (20 × 20 rooms, 6344 meshes, 14580
  cells of 3.9 m, each sampled as eight 2 m cubes) in about 12 minutes into 11.8 MB,
  with the process peaking under 0.6 GB, about what the occluders' BVH alone takes
  (part 2 took some 8 hours on that level). Fewer cores take longer in proportion.
  These figures sample every cell. What costs is the cells under the ground floor
  and across a storey's slab, where only many faces together hide what is past them;
  a level with a floor samples only the cells over it, and the bake report's
  `layerSeconds` says what each layer of the grid took.
  A level of imported meshes costs about what one of boxes does: a 6 × 6 town of
  tessellated one-mesh buildings on bumpy ground (5120 cells of 2 m) bakes at
  about 700 cells a second, its box-built twin at about 550 (#1981; it was 78
  before tessellated faces fused). Moved and scaled off round coordinates
  (6125 cells), the town bakes at about 560 cells a second against its twin's
  650, casting 2.2 times the rays: a tessellated wall fuses to the texel at its
  edges and openings, where a box's own face is proved exactly. A cell's cost grows with the meshes the level
  holds, slowly where they crowd (5.6 times the meshes, about half the rate) and
  about in proportion to the level's floor area.
  At runtime a camera costs one cell lookup, and each static mesh
  a map lookup and a bit test; a static mesh moved or edited re-checks that mesh alone.
- **Baked alone** by `Lighting.BakeOcclusion()` or the scene inspector's **"Bake
  Occlusion"** button beside Generate Lighting (#1958; Unity bakes occlusion from its
  own window): the same step on the open scene, written beside it, with no lightmap
  or probe touched and the scene file left as it is. Refused as Generate Lighting is:
  in Play, while the editor's bake runs, and for a scene never saved. A level whose
  full Generate takes most of an hour re-bakes its PVS in what *Cost* above says:
  seconds for the primitive bench at Kolvik's size, minutes for a Dust2-sized
  level. (Kolvik itself took 20 minutes before #1981; its time since is not yet
  measured.)
- **Kept when unchanged** (#2348): Generate Lighting re-bakes no PVS when nothing
  its bake reads changed since the PVS the scene holds was baked: the same static
  meshes, in the same order, each with the same fingerprint (*A bake older than the
  scene*, below) and occluder flag, baked by the same version of the bake, from the
  same sampled cells (#2328: the same floor source and play height, and the same
  cells over its floor and holding a camera or a shadow-casting light). A lamp moved
  (a shadow-casting one within its cell), a light's intensity changed or a probe
  added keeps it; a shadow-casting light or a camera moved to another cell, a changed
  agent or jump height, or a navmesh re-baked so a camera reaches other cells,
  re-bakes it. A kept PVS's file stays too,
  byte for byte: it is exactly what a re-bake would write (the bake is seeded). The
  kept PVS owns its file as a bake does: one another process wrote there since is
  overwritten with it, and the warning says so. The step then takes about the time to
  fingerprint the static meshes (no triangle is gathered); `Lighting.Generate()` logs
  `[Lighting] 9:41 occlusion unchanged, kept` as its step line, and the editor's
  Generate Lighting's summary line reads `Generated lighting: 3 lightmap(s), 12 light
  probe(s), 2 reflection probe(s), occlusion unchanged, kept; saved` (or `; save the
  scene to keep it` in place of `; saved` on a scene not yet saved). The
  `occlusion` bake report keeps the PVS's counts, its stages `skipped: unchanged` and
  then `write` (when the file is written beside the scene), and `Lighting.Generate()`
  returns the kept PVS's counts. The report's `settings` carry the bake's `version`,
  bumped whenever the bake would write other bytes for the same objects, so an engine
  update re-bakes. A scene
  with no PVS, one baked from anything else, or a level too large to cull bakes as
  before. **`Lighting.BakeOcclusion()` and the "Bake Occlusion" button always re-bake.**
- **Runs** as Generate Lighting's last step, after the lightmaps and probes are
  written into the scene, so a bake dropped or failed during it (the scene closed or
  reloaded, another opened) loses none of the lighting: in the editor on a worker
  thread, with its cells on the card's bar, so the editor and the MCP keep answering;
  `Lighting.Generate()` bakes it in line and logs `[Lighting] occlusion 62%
  (2260/3645 cells)` lines like the other steps.
- **Stored** beside the scene as `<scene>.occlusion` (a small binary file, written
  by the bake, and by a save only by the rule in
  [*Bake files and the scene save*](#bake-files-and-the-scene-save)). A file that
  is truncated or out of range is refused on load with a warning, and nothing is
  occlusion-culled, and the next save removes or rewrites it. A file another
  version of the bake wrote is refused by its version, read first: `<file> is an
  older occlusion bake (version 1, this build writes 2): nothing is
  occlusion-culled until it is re-baked (Generate Lighting or Bake Occlusion)`; a
  file from before the version field (whose first number is its object count) is
  told as `an occlusion bake from an older engine, which wrote no version`, however
  many objects it holds. One that is there but
  cannot be read (a permission) also culls nothing, with a warning, and the next save
  leaves it in place.
- **A bake older than the scene** never hides what is there: a static mesh moved,
  re-shaped (the bake hashes each mesh's triangles, corner by corner), given a
  material that occludes or faces differently (opaque or not, a surface shader, its
  Render Face), added to or taken out of a `LODGroup`, or made dynamic since is
  drawn, a mesh added since is drawn, and if an occluder changed so, or was removed
  or deactivated, nothing is occlusion-culled until the next Generate Lighting or
  Bake Occlusion. The console says so once, and again if an occluder changes after
  it said only meshes did. Only what the bake reads counts (#1958): a material
  renamed, and a re-export that stores the same triangles in another vertex order,
  keep the bake. The bake reads the live material library and the next load reads
  the file's, so a material a script defined in memory that the scene file lacks
  keeps the bake only when it reads as the default does (opaque, front faces); one
  that is two-sided, Cutout, transparent or has a surface shader does not. So
  `Lighting.Generate()`, `Lighting.BakeOcclusion()` and the editor's two buttons
  warn once, naming the keys, when the file lacks, or holds otherwise, a material
  the baked meshes draw: save the scene so its materials are written. A runtime `Scene.Load` that moves the level's
  ids out of a `DontDestroyOnLoad` entity's way moves the bake's with them.
- **Per camera.** Each perspective camera reads its own position's cell: the base
  camera, a stacked or viewmodel camera, a render-texture camera. An orthographic
  camera (a minimap) sees from a whole plane, not its eye, and a camera whose near
  plane is past 1 m (half a cell) clips walls away, so neither culls by occlusion.
  Probe and reflection bakes cull nothing. A camera whose culling mask leaves out an occluder's layer sees past
  it, so it culls nothing by occlusion. A selected mesh the editor's outline rings is
  always drawn.
- **Per light** (#1714, part 4): a shadowed point or spot light reads the cell its
  own position stands in, and leaves out of its shadow maps the static casters that
  cell cannot see. The camera's view plays no part (a caster the camera cannot see
  may still cast into view). A caster hidden from the light is behind a wall the
  same map draws, so its shadow is the same without it, with the camera's caveat
  (*What it can miss*): a caster the light sees only through a gap showing less
  than about a metre of it may be left out, and lose its shadow through that gap.
  Unlike a camera's miss, which comes back as the camera moves, a still light's
  cached tile keeps it until the light moves or a static caster changes. So a light culls nothing by occlusion while a
  baked occluder casts no shadow (`Mesh` cast mode `Off`) or has no GPU buffers, when
  it is shadowmasked under `Shadowmask` (its walls are in the mask, not the map), or
  stands outside every cell; the sun's cascades never do. It pays where a light's
  map is drawn: a moving shadowed light (a carried flashlight, a swinging lamp) every
  frame, a still one when its static bake re-bakes.
- **See it** in the editor (Unity's *Visualization*): select a camera that culls by
  occlusion, and the Scene
  view outlines the cell it stands in (yellow) and every static mesh the bake hides
  from it (red). `make editor-capture ARGS="--select <camera>"` renders the same.
- **Measure** with `Debug.Stats().occlusion_culled` (per frame, summed over the
  camera views) and `shadow_occlusion_culled` (over the shadow views).

### GPU occlusion culling

Unity 6's *GPU Occlusion Culling* (#1963): with no bake, wherever the camera stands
(outside the baked cells, in the editor's Scene view far from the play area), a
camera with `use_occlusion_culling` on leaves undrawn the opaque meshes that what it
drew hides. The quality preset's `gpu_occlusion_culling` turns it on (off in every seeded
preset, opt-in until it is measured to pay on the project's scenes; [`Graphics`](Graphics.md), *Quality*). It changes no pixel: a mesh it skips
is wholly behind depth the camera drew this frame.

- **How.** Two phases per camera, the standard way: the meshes it drew last frame
  draw their depth first; that depth is reduced into a pyramid of farthest depths;
  every opaque mesh's bounds are tested against it on the GPU, at the pyramid level
  where its screen rect spans at most 2×2 texels; the camera's passes then draw the
  ones that passed. A mesh that came out from behind a wall this frame is drawn this
  frame. A mesh whose bounds cross the camera's near plane is always drawn.
- **Beside the bake.** The baked PVS (*Occlusion culling*, above) still runs first
  where it is usable, as Unity keeps Umbra next to the GPU Resident Drawer: what its
  row hides is never tested. Where the bake is missing, older than the scene, or the
  camera stands outside every cell, the GPU test alone culls.
- **What takes part.** Opaque and Cutout meshes, static or not; a skinned mesh is
  tested with its posed bounds. Only what the SSAO depth prepass draws hides
  anything: lit solids, a Cutout texel clipped, a custom shader's own cut or moved
  vertices. A Transparent mesh hides nothing and is always drawn, as are a mesh
  whose bounds aren't known yet, a mesh particle, a selected mesh the editor's
  outline rings, and a mesh whose custom shader cuts, scrolls or moves its
  vertices (`vertex_waves` swells rise out of the mesh's own bounds, so a water
  plane behind a low rim would vanish though its crests show; it still hides
  what is behind it).
- **Which cameras.** Each camera view, its own: the base camera, a stacked or
  viewmodel camera, a render-texture camera, the editor's Scene view. Unlike the
  bake, an orthographic camera and one whose near plane is far out cull too: the test
  reads the depth the camera drew, so a wall its near plane clips away hides
  nothing. Probe and reflection bakes, and a GPU without indirect first-instance
  draws, cull nothing this way. Nor does a camera whose cull would test fewer than 32 mesh
  entities this frame (a viewmodel camera drawing a gun), unless it draws a
  clustered mesh (*Per cluster*, below) with `gpu_cluster_culling` on: culling so
  few costs more than it could hide, so it draws them all, as with its
  `use_occlusion_culling` off, except that where depth priming runs (never on
  macOS) a depth prepass of those few still primes its forward pass, as it did
  while it culled, so overlapping arms and gun shade each pixel once. The floor counts the mesh entities the cull tests
  (mesh particles, a vertex-moving material and an outlined object are never
  tested, so they do not count), not whether the camera is stacked, so an
  overlay showing the whole level still culls.
- **Cost.** Per camera, a depth-only draw of last frame's set, a compute reduction
  of the depth, and one compute test per opaque mesh; the draw calls stay the same
  (indirect). That first depth is kept: the depth prepass (SSAO's or SSIL's, or,
  with neither, the one that primes the forward pass where depth priming runs,
  never on macOS) then draws
  only the late set, what the test found visible that the first draw left out, so
  a still view pays for its depth once (#2336). The meshes still go through the CPU's frustum and size culls first.
- **Per cluster** (#1968), the culling half of UE5's Nanite: a static mesh of at
  least 4096 triangles is split at import into clusters of up to 124 triangles,
  each with its bounds and normal cone, kept with the model. After its whole mesh
  passed the test above, each cluster is tested on its own, by the frustum, by
  whether every triangle in it faces away, and by the same pyramid, and only the
  ones that pass are drawn: the half of a canopy or a building shell a wall hides
  is left out. A cluster crossing the near plane is always drawn; a two-sided or
  back-faced material, or a mirrored or non-uniformly scaled instance, takes no
  facing test; a skinned mesh is never split. A mesh whose custom shader moves
  its vertices is split and reordered at import like any other, but the instance
  test never judges it, so its clusters are never tested and it keeps one
  instanced draw; mesh particles likewise. It changes no pixel either. A
  camera drawing a split mesh culls by cluster however few entities it shows,
  past the instance floor of *Which cameras* above: one big shell gains by
  cluster whatever the count. It is its own switch, the
  preset's `gpu_cluster_culling` (off in every seeded preset), and does nothing
  without `gpu_occlusion_culling`: a preset that wants it names both
  ([`Graphics`](Graphics.md), *Quality presets*). A split mesh is
  drawn once per instance, not instanced, so many copies of one big mesh cost a
  draw each; past a per-camera budget (8M indices) a batch is tested whole. Every split
  mesh resident in the session shares one cluster table and one index pool, each
  held under the GPU's storage-binding limit (128 MiB on most devices: about 33.5M
  indices or 2M clusters in all): the meshes of the scenes loaded now, since a load
  that replaces the scene releases the meshes only the scene before used. So the
  limit is on the total, not on one mesh: a mesh that would pass it logs `does not
  fit the cluster pool under the device's storage binding limit; it is drawn whole`
  and is culled only as a whole instance until a later load's release frees room,
  when it is leased again (the next upload of a mesh only a preview or a mesh
  particle draws). `Debug.Stats().cluster_pool_bytes` reads what is leased.
- **Measure** with `Debug.Stats().hiz_tested`, `hiz_culled` and `hiz_late` (read
  back a frame or so late), and `cluster_tested` / `cluster_culled` for the clusters;
  `draw_calls` and `triangles` count what its passes drew, phase one's depth apart in
  `hiz_depth_triangles` and the per-instance draws of a clustered batch in
  `cluster_draws` (#2387). What it
  costs is `gpu_passes.occlusion` and, in a bench, its parts' rows
  (`gpu.occlusion.<part>`, `render.occlusion.<part>`, [`docs/testing.md`](../testing.md)).

### Bake reports

Every bake writes what it measured and what it found **beside the scene** (#1980), one
JSON file per kind: `<scene>.bake/lightmaps.json`, `probes.json`, `reflections.json`,
`occlusion.json` and `navmesh.json` ([`Navigation.md`](Navigation.md)). The next bake of
that kind overwrites it, so two bakes compare by copying the file between them;
Generate Lighting writes the ones for the steps it ran. Only a bake writes one, never a
scene save; nothing reads them at runtime, and an export leaves the `.bake` folder
behind. A report that cannot be written is a console warning, never a failed bake. A bake
of an unsaved scene (`Lighting.Bake()` before the first save) writes none.
`Lighting.GetBakeReport(kind)` returns the file as a table, so a scenario can assert on a
bake: `assert(Lighting.GetBakeReport("occlusion").visibility.culledFraction >= 0.4)`.
A value the bake could not measure is `null` in the file and `nil` in the table.

Every report opens with the same fields:

| Field | What it holds |
|---|---|
| `kind` | the report's kind |
| `engine` | the engine commit that baked it (the one the project file records) |
| `timestamp` | when it was written, Unix seconds |
| `threads` | what the bake ran on: the CPU workers it used for `lightmaps` and `occlusion` (every core), `1` for `navmesh` (one thread), `"gpu"` for `probes` and `reflections` (a GPU capture) |
| `stages` | `{ name, seconds }` per stage, in order; `wallSeconds` is their total |
| `settings` | the settings the bake used |
| `inputFingerprint` | a hash of every input the bake read, hex: the same scene and settings give the same one, and a change to anything the bake reads (for the probes and reflections: the static geometry, its materials' colours, emission and textures, the lights, ambient, sky, skybox, fog, the lightmaps and the lighting settings) gives another |
| `outputBytes` | the bytes it wrote (for the light probes, the SH they carry: their sidecar is written with the scene's lighting) |

Then each kind's own:

- **`lightmaps`:** `maps`, `pageSize`, `pages` (`{ page, maps, occupancy }`, the share of
  the page's texels its maps cover), `meshes`, slowest first (`{ entity, name, page, size,
  texels, charts, seconds, texelsPerSecond, lightsInReach, requestedDensity,
  actualDensity }`: `seconds` is worker time over every thread, the densities are texels
  per metre asked for (texels per unit × Scale In Lightmap) and got), `zeroScaleInLightmap`
  (the meshes left out at Scale In Lightmap 0), `receiveLightProbes` (the meshes left
  out because their Receive Global Illumination is `LightProbes`), `denoise` (`{ charts, seconds }`, `nil`
  without OIDN), `peakResidentMB` (the process's peak resident memory when the bake
  finished, `nil` where it cannot be read: in the editor it counts the editor's own)
  and `lights` (`{ index, kind, position, range, bakesDirect, texelsInReach }`).
  A light is in reach of a mesh when the mesh's bounds meet the sphere of its range (a
  directional light reaches every mesh): a spot light's cone is not read.
  `invalidTexels` (the backface texels the bake filled) is `nil`: only the tracer knows it.
- **`probes`:** `count`, `manual`, `bounds` (`{ min, max }`), `bounces` (the wall time of
  each bounce pass), `sampling` (`"grid"`, trilinear, or `"nearest"`, the four nearest
  blended: there is no tetrahedral mesh, so no tetrahedra to count) and `farObjects`
  (`{ count, list }`: the active dynamic drawn objects whose centre (as the renderer takes
  it: the mesh's local bounds under its world matrix, halved) is over
  8 m from the nearest probe, farthest first, each `{ entity, name, centre, nearest }`).
- **`reflections`:** `count`, `probes` (`{ index, position, shape, boxMin, boxMax,
  rotation, radius, resolution, importance, manual, bytes, seconds, overlaps }`, `shape`
  `"Box"` or `"Sphere"`, a box carrying `boxMin`, `boxMax` and `rotation` (a quaternion,
  `[x, y, z, w]`) and a sphere `radius`, `resolution` the face size the bake rendered at,
  `overlaps` the probes whose volume (the turned box or the sphere) this one's overlaps)
  and `picks`: **which
  probe each active drawn object reflects**, at its bounds' centre as the renderer takes
  it (the mesh's local bounds under its world matrix) (`{ entity, name, centre, probe, weight,
  second, secondWeight }`, `probe` `nil` for the sky). The picks rank the scene's whole
  set: a probe the memory budget leaves out at runtime is not skipped here.
- **`occlusion`:** `cells`, `bakedCells` and `unbakedCells` (of them, the cells
  sampled and the cells left seeing every object), `floorSource` (`"navmesh"` or
  `"everywhere"`: where the bake found the floor, *Occlusion culling*),
  `navmeshMissed` (`true` when a current navmesh marked no cell of the grid, so
  every cell was sampled), `floorCoverage` (the share, 0 to 1, of the static meshes'
  footprint the navmesh's floor covers; `nil` when the bake had no floor, a kept one
  too), `settings.playHeight` (how far over the floor a camera can be, metres, from
  the navmesh settings; 0 with no floor), `cellSize`, `tooLarge`, `rows`, `dataBytes`,
  `objects`, `occluders`, `neverCulled`, `occluderTriangles`, `biggestOccluders`
  (`{ entity, name, triangles }`, at most 20) and `visibility`, taken over the cells a
  camera can be in (every cell when every cell was sampled; the air left seeing
  everything is not counted, so a level culls in these figures as a bake of every
  cell reports it): `cells` (how many it counts), `meanVisible` and `maxVisible`
  (objects such a cell sees), `culledFraction` (the share of the objects such a cell
  culls, on average), `alwaysVisible` (how many objects every such cell sees; the never
  culled are among them) and `alwaysVisibleNames` (at most 20). `neverCulled` is a count with no names of its own:
  which objects the bake could not sample is known only inside it. `cellSeconds` (`{ mean,
  max }`) is the time a cell took to sample, and `slowestCells` the 20 slowest, slowest
  first (ties to the lower cell): `{ cell, centre, seconds, visible }`, `cell` its index in
  the grid, `centre` its middle, `seconds` its time to the microsecond and `visible` the
  objects its PVS row sees. `layerSeconds` is each grid layer's time, bottom layer first:
  `{ layer, minY, maxY, cells, seconds, mean }`, the heights it spans and the sum and mean
  of its cells' times, so the air over a floor is costed apart from the floor's own. A cell
  runs on one worker, so its time is that worker's, read on
  the platform's clock: the sampling itself never reads one, and a timed bake writes the
  same PVS as an untimed one. All three are `nil` when no PVS was kept, or when the bake was
  not timed cell by cell, as for a PVS Generate Lighting kept unchanged, whose
  bake stage is `skipped: unchanged`.

A stage baked in the editor a step a frame (the probes) counts the frames between its
steps too. Lists of "heaviest first" keep the top 20; the counts hold them all.

### Environment

The scene's sky and ambient light (#1612), Unity's `RenderSettings.skybox`,
`ambientLight`, `ambientIntensity` and `ambientMode` with its sky, equator and ground
colours (#1647), and the procedural sky's look (#1620),
Unity's *Skybox/Procedural* tint and ground with HDRP's *Cloud Layer* coverage and
opacity. They are the scene file's `skybox_path`, `ambient_color`,
`ambient_intensity`, `ambient` and `sky` blocks, the same fields the editor's **Scene
Settings** panel (the Inspector with nothing selected) edits, through the same
write: **save the scene** to keep a change. Unlike the bakes they are in every build,
so a game script can drive them too (a day cycle, a mood change); a change made
during Play goes back on Stop, as every scene edit does.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.GetSkybox` | `()` | the panorama's path, `""` for the procedural sky |
| `Lighting.SetSkybox` | `(path)` | — |
| `Lighting.GetAmbientColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientColor` | `(r, g, b)` | — |
| `Lighting.GetAmbientIntensity` | `()` | number |
| `Lighting.SetAmbientIntensity` | `(intensity)` | — |
| `Lighting.GetAmbientMode` | `()` | `"Color"`, `"Gradient"` or `"Skybox"` |
| `Lighting.SetAmbientMode` | `(mode)` | — (an unknown name errors) |
| `Lighting.GetAmbientSkyColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientSkyColor` | `(r, g, b)` | — |
| `Lighting.GetAmbientEquatorColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientEquatorColor` | `(r, g, b)` | — |
| `Lighting.GetAmbientGroundColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientGroundColor` | `(r, g, b)` | — |
| `Lighting.GetSkyZenithColor` | `()` | `r, g, b`, or `nil` while it follows the ambient colour |
| `Lighting.SetSkyZenithColor` | `([r, g, b])` | — |
| `Lighting.GetSkyHorizonColor` | `()` | `r, g, b`, or `nil` while it follows the zenith |
| `Lighting.SetSkyHorizonColor` | `([r, g, b])` | — |
| `Lighting.GetSkyGroundColor` | `()` | `r, g, b`, or `nil` while it follows the zenith |
| `Lighting.SetSkyGroundColor` | `([r, g, b])` | — |
| `Lighting.GetCloudCoverage` | `()` | number, `0`–`1` |
| `Lighting.SetCloudCoverage` | `(coverage)` | — |
| `Lighting.GetCloudOpacity` | `()` | number, `0`–`1` |
| `Lighting.SetCloudOpacity` | `(opacity)` | — |

- **The skybox** is an equirectangular panorama image, its path written as the
  Scene Settings panel takes it (`"assets/textures/sky.png"`). The next frame loads it and re-captures the
  sky's reflection cube from it, so reflections follow. `""` (or `nil`) goes back to
  the procedural sky (below). A path that loads
  nothing draws the renderer's fallback texture, as it would from the scene file.
- **The ambient colour** is linear RGB, each channel clamped to `≥ 0`; a colour with
  a NaN or infinite channel is refused and changes nothing. **The intensity** scales
  it, clamped to `≥ 0` (NaN or `math.huge` reads as 0; the editor's slider stops at
  `5`, a script may go past it). Together they are the flat hemispherical ambient a
  surface takes where no baked probe or lightmap reaches. The procedural sky is
  tinted by the colour alone: the intensity dims or brightens what the ambient lights,
  never the sky itself, as Unity's `ambientIntensity` leaves the skybox. An active **Ambient**
  light entity, when the scene has one, overrides both (see [`Light`](Light.md)),
  and the mode below with them: it always lights as `Color`.
- **The ambient mode** is Unity's *Environment Lighting → Source*. `"Color"` (the
  default) is the hemisphere above: the colour on up-facing surfaces, fading to a
  quarter of it on down-facing ones. `"Gradient"` (Unity's `AmbientMode.Trilight`)
  takes three colours instead: **sky** on up-facing surfaces, **equator** on
  sideways ones and **ground** on down-facing ones, blended by how much a surface
  faces up or down. The intensity scales all three. The colours follow the ambient
  colour's rule (linear RGB, `≥ 0`, a NaN or infinite channel refused) and are kept
  in the other modes; their defaults are the default ambient's hemisphere at its top,
  middle and bottom, so switching the default scene to `"Gradient"` lights it the
  same. `"Skybox"` (Unity's `AmbientMode.Skybox`, Unity's default; rusty's stays
  `"Color"`) lights the scene from the sky itself: the panorama (`SetSkybox`) when
  one is set, else the procedural sky below, projected into three SH bands
  (Unity's *ambient probe*), so an HDRI's blue zenith and warm horizon light up-
  and sideways-facing surfaces with their own colours. The intensity scales it; the
  sky on screen stays as drawn. The projection is made once per change of the sky
  (a different panorama path, the panorama file or its import settings rewritten, a
  procedural setting) and never saved; a rewrite is seen as a texture's is: one the
  engine makes, or one the editor's file watch sees, while a shipped game re-reads
  nothing; a scripted ambient colour tinting the procedural sky re-projects
  nothing. A panorama is read as the sky pass draws it (8-bit, no sRGB decode), and
  one that cannot be read lights as the procedural sky does. A scene in `Color` mode
  with default colours leaves the `ambient` block out of its file. Lightmaps and light probes, both Generate Lighting's (`Probe.Bake`)
  and the analytic fill (`Probe.BakeAnalytic`), bake from the same ambient, so an
  open lightmapped or probe-lit surface takes the light an unbaked one facing the
  same way does: exactly in `Color` mode, in `Skybox` mode while the sky's three
  SH bands stay non-negative, and in `Gradient` up to a slight smoothing at the
  horizon (the bakes carry three SH bands, as Unity's ambient probe does). A sky
  with one dominant bright region (a sun-white top over a black ground) rings below
  zero on the far side; the bakes clamp each ray at zero, so a baked surface turned
  away from that region comes out slightly brighter than an unbaked one, which
  clamps once (straight down under that sky: about `0.07` against `0`). As in Unity, light probes and lightmaps bake against this
  ambient, never against the sky the camera draws (#1668): in `Color` and
  `Gradient` the sky reaches surfaces only through reflections (reflection probes
  capture it), so a panorama scene lights its probes and lightmaps from the flat
  ambient, not from the HDRI; in `Skybox` the ambient *is* the sky's light, so the
  HDRI lights unbaked, lightmapped and probe-lit surfaces alike (#1675). But, as
  in Unity, **a changed mode or colour reaches baked surfaces only at the next
  Generate Lighting**: dynamic objects and unbaked surfaces show it at once.
- **The procedural sky** is drawn when no panorama is set: a ground → horizon →
  zenith gradient under a cloud layer. Each colour is linear RGB (channels clamped
  to `≥ 0`; a NaN or infinite channel is refused and changes nothing) and
  **optional**: unset (the default, or a setter called with no arguments) the
  zenith is the ambient colour, the horizon the zenith lifted 60% toward white,
  and the ground a quarter of the zenith, so a scripted ambient colour still tints
  them every frame. A set colour stays put whatever the ambient does.
  **Cloud coverage** (default `0.5`) runs from `0`, a clear sky, to `1`, overcast;
  **cloud opacity** (default `0.6`) from `0`, no clouds, to `1`, solid ones. Both
  clamp to `0`–`1`; NaN or `math.huge` changes nothing. The defaults draw the sky
  rusty has always drawn, and a scene at its defaults leaves the `sky` block out of
  its file. What a surface reflects follows: the sky's reflection cube is re-baked
  once per change, so a script that changes a setting every frame re-bakes every
  frame (the ambient colour never re-bakes it), and reflection-probe bakes capture
  the sky as drawn. In `Color` and `Gradient` mode the ambient a surface takes is
  the ambient colour (or the gradient's colours), not the sky's: the sky's colours
  light nothing, as Unity's skybox lights nothing in those modes. In `Skybox` mode
  they are the light (clouds included, by their average brightness).
