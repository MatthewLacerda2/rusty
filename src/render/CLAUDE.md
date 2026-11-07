# CLAUDE.md — the renderer

This file loads when you work under `src/render/`. The shaders live in
`engine/shaders/`, which has no CLAUDE.md of its own: **read this file before editing
a `.wgsl` there too.** The root `CLAUDE.md` keeps the project-wide rules;
`docs/testing.md` is the reference for GPU tests.

## Where render sits
- **Platform layer, downstream of the sim.** Render may read the sim modules its row in
  `tools/lint/src/layers/table.rs` lists; no sim module references `crate::render`,
  `wgpu` or `egui` (`make direction`). What the sim needs from shaders (validating a
  bake, say) lives in a sim module render reads, as `shadergen` does, never in a call
  up into render.
- The module root opts out of the clock ban because GPU timing is wall-clock (#757);
  the opt-out stays at the root, never in a file below it.

## The retained scene
- **A camera's mesh solids come from `render::scene`, not a walk of the ECS** (#1706). The
  extract (`Renderer::extract_scene`, once per frame in `prepare_frame`) keeps one
  `RenderObject` per mesh entity per scene, updated from the change log
  (`ecs::changes`): a move re-places the object, any other change re-derives it, and a
  frame that changed nothing walks no object. A camera only culls the objects and
  adds what it decides itself. Batch keys name a mesh by its interned `MeshHandle`;
  `Renderer::gpu_mesh_of` resolves one.
- **A new per-object input is kept or read per camera, on purpose.** Kept: derive it
  in `extract_object` or `pose_object`, and make sure whatever changes it re-derives
  the object (a change channel, a scene value's revision read in `scene/inputs.rs`
  (`scene::revision`), or
  `Retained::touch` at the renderer-side write, for an input every key reads). Per
  camera: read it in `scene/draw.rs`. `scene/inputs.rs` lists every input by which
  way it is held. Add a case to `scene/equivalence_tests.rs`, which holds that an
  incrementally kept scene equals one built from scratch.
- **The shadow sweeps gather from the objects too** (part 2 of #1706): a caster's
  cast mode, clips and static-bake terms are kept on its `RenderObject`, and the
  static bakes' key is `RenderScene::statics_key`, refolded only when a static object
  was re-derived. A new input a shadow reads is kept or read per frame the same way,
  and `passes::shadows::statics_key` (the from-scratch digest the tests compare)
  folds it too.
- **What an object holds lives on the GPU in its slot** (part 3 of #1706): its
  `ObjectData` (world matrix, probe SH, lightmap) sits at its slot of the object table
  (`scene/table.rs`), written where it changed (`gpu/draw_buffers/objects.rs`). A
  camera's `InstanceData` is 16 bytes naming the slot plus what the camera decides
  (reflection probes, proxy volume, motion history in the joint array). A new
  per-object value the shader reads goes into `ObjectData` (Rust and
  `forward/bindings.wgsl`), written in `pose_object`; a per-view one into
  `InstanceData`, and the fragment stage gets it through `VertexOutput::instance`,
  since it is at its 8 storage buffers and never binds the instances.
- **A material's record lives on the GPU in its slot too** (part 4 of #1706): one
  `MaterialData` per (scene, library material, `Variant`) in the material table
  (`scene/records.rs`), written where it changed; a batch key names the slot
  (`BatchKey::record`) and carries what the CPU decides a pass by
  (`BatchKey::traits`). Each frame names a slot by its value class's least slot
  (`scene/classes.rs`, `MaterialRecords::rep`) before it batches, so equal records
  under different names (each entity's own `Material.Set*` material) share a draw;
  a record is written only through `MaterialRecords`, which keeps the classes. A library edit that moves only a record's values (a colour,
  a scalar, the params: `records::rekeys` false) rewrites its records and param buffer
  in place and re-derives no object; any other edit re-derives only the objects naming
  that material (`RenderScene::by_material`). A record never reads the entity: a
  per-object input is a `Variant`, the object record, or the instance.
- **Occlusion culling reads the scene's baked PVS** (#1714, `scene/occlusion/`):
  bound per scene and re-checked against a baked object's fingerprint only after
  that static object changed (a whole bind when the PVS, the objects or the
  `LODGroup`s' renderers changed); it may keep what is hidden and never culls what
  a camera sees. A new path that decides what a camera draws reads it through
  `Renderer::occlusion_eye`, so a capture, an orthographic camera and a far near
  plane cull nothing. A light's shadow views read it through
  `OcclusionBinding::shadow_row` (the light's cell, culling nothing while a matching
  occluder casts no shadow) and only for a tile whose statics are drawn into it.

## Shaders
- **Shared WGSL is a naga_oil composable module, never a copy.** WGSL has no
  `#include`. Every shader composes through `shadergen::compose`, the one GPU-free
  composition path (#722), which registers `common.wgsl` as `"common"`; a shader pulls
  it in with `#import "common"`. When two shaders need the same struct or function,
  move it into a module rather than pasting it (#1063 splits the forward shader this
  way).
  The fullscreen triangle is one: `#import fullscreen`, and `make rules` flags a copy (#1370).
- **Derivatives only in uniform control flow**: `dpdx`, `dpdy`, `fwidth` and an
  implicit-LOD `textureSample` sit before any branch on a per-pixel value, as
  `fold_decals` and `ui.wgsl`'s `shade` do. Inside one, a quad neighbour outside the
  branch leaves them undefined, and on Metal SSR's normal changed frame to frame (#1525).
  No gate holds it: naga's validator skips the fragment stage's uniformity check, and
  a text match can't tell a per-pixel branch from a uniform one.
- `render::gpu::shaders::ShaderRegistry` reads shaders from disk at runtime, so a
  shader edit needs no Rust rebuild.
- wgpu, naga and naga_oil upgrade as a unit (`Cargo.toml`).

## Reuse `render::gpu`
Before building a layout or pipeline by hand, use what #939 extracted:
- `render::gpu::layout_entries`: one uniform, texture, sampler, depth or storage entry.
- `render::gpu::DEPTH_FORMAT`: every view depth target and every pipeline drawing into
  one; shadow maps keep their own format in `passes::shadows`.
- `render::gpu::fullscreen::Fullscreen`: any fullscreen-triangle pass (post-FX, SSAO,
  the UI blur, the shadow-atlas blit).
- `render::passes::blended`: the globals uniform and the alpha and additive pipelines
  into the HDR target that test depth without writing it (particles, ribbons).
- `render::gpu::pipelines::LazyPipeline`: a pipeline a frame may not draw (an
  editor overlay, a blend mode, a pass only some scenes use) is built on its first
  draw, not with the renderer. On lavapipe each pipeline is an LLVM compile, and every
  GPU test builds its own renderer (#985). A lazy pipeline must be listed in
  `Renderer::lazy_pipelines` (`render::setup::warm`), so `warm_pipelines` builds it
  when a windowed renderer starts: a game never compiles one mid-frame.

## Adding a post effect
A post effect is a module in every layer (#1467): it adds its own files and **one
row per registry** (each folder's `mod` list counts as one), and edits no shared
struct, shader body or list position. Group a family (`grading/`, `lens/`) the same way
in every layer. The test-only `dummy` effect is the worked example: grep `dummy` for
every file and row.
- **Settings** — `components/rendering/post_processing/<effect>.rs` (serde, `Default`,
  the clamp constants) and one field on `PostProcessingComponent`, plus its line in
  that struct's `Default` (the compiler asks for it: the volume applies by default, so
  the impl is written out). A camera effect keeps its knobs on `CameraComponent`,
  named `<effect>_…`.
- **Ops** — `scene/authoring/rendering/post_processing/<effect>.rs`: its ops, its
  `Effect` (its field-table row through `effect!`, and its `Knob`s, which give the
  Inspector's sliders, `Graphics.Get/Set<api>` pairs and the `Graphics.<effect>.<knob>`
  tween paths) and its row in `EFFECTS`.
- **API** — `api/graphics/effects/<effect>.rs`, a `register` (knobs through
  `knobs::register`), and its row in `REGISTRARS`.
- **Inspector** — `editor/inspector/components/camera/post_processing/<effect>.rs`, a
  `Section` with its display `order` and `group`, and its row in `SECTIONS`
  (`CAMERA_SECTIONS` for a camera effect).
- **Render** — `render/postfx/effects/<effect>.rs` (or a folder) and its row in
  `POST_EFFECTS`: composite functions (`Phase::Hdr` before the grade, `Phase::Display`
  after it), grade functions (the LUT bake) and/or passes (`Pass::Scene` before the
  composite, `Pass::Ldr` after it; TAA has one of each), **each placed by the order
  it declares**. Its WGSL
  is a module at `engine/shaders/postfx/effects/<effect>.wgsl` with
  `#define_import_path postfx::effects::<effect>`: a function takes the colour, the
  `Frag` (composite only), one `texture_2d<f32>` per composite texture and one `vec4`
  per slot it declares, reads the chain's bindings from `postfx::frame`, and leaves
  the colour as it was when its slots say it is off. Its `pack` writes only its own
  slots. A pass with pipelines of its own adds them to `effects::Passes` (and lazy
  ones to its `lazy_pipelines`). Its per-view targets are its own type in
  `PostFx::targets` (`effects::Targets`: `get_or` while it runs, `remove` when it
  doesn't; a resize drops every slot), never a `PostFx` field.
- **Size** — a pass after TAA's (a `Pass::Scene` ordered after 15, and every
  `Pass::Ldr`) sizes its targets by `PostFx::chain_size`, **never** `full_size` or
  `RenderView::scene_size()`, and refits when it changes. The two differ only while
  TAA upscales the view (TAAU, #1702: a render scale below 1 with TAA on), when the
  chain after TAA runs at the output's size, so a pass sized by the scene passes
  every other test and goes wrong only there. A pass that reads the velocity
  target or depth (scene-size) from a chain-size image maps its pixel by the size
  ratio, as `motion_blur.wgsl`'s `velocity_at` does. A pass before TAA (depth of
  field, SSR) sizes by `full_size`. `effects::chain_size_tests` scans each later
  pass's files and fails on a scene size; a line that must read one says so inline
  (`chain-size: scene <why>`).
- **Composite textures** — a composite function that reads a texture of its effect's
  own (a target its pass drew, a texture it picked) declares it in its `textures`
  ([`CompositeTexture`](postfx/effects/textures.rs): a name and a `view` fn of the
  `PostFx`). The registry binds it in the composite's group 1 and passes it to the
  function (`effects::textures`, #1473), so no effect edits `postfx/frame.wgsl`, the
  io layout or `PostFx::run`. A `view` of `None` binds one shared black texel, so the
  function must draw nothing when it reads black (bloom's dirt, the lens flare), and
  an effect that is off skips the fetch on a slot's on flag, as bloom and the flare
  do (`p.w`), so per pixel it costs a uniform branch, not a texture read. It samples with the chain's own samplers (`s_color`, `s_skybox`). The `dummy`
  declares one, never provided.
- **Gate** — its `PostEffect::gate`: the player option, when it is player-facing.
  It asks through `EffectCx::runs(EFFECT.gate, on)`; no shared `match` names it. No
  quality preset gates an effect (#1522).
- **Quality** — an effect with a cost knob carries an `EffectQuality` selector and its
  `Custom` values on its settings, and its row in `core::settings::quality::QualityValues`
  (three `Levels`); its render code picks with `quality.pick(&graphics.quality.<row>, custom)`.
- **Option** — a player-facing effect adds its `GraphicsOption` row, named as the
  effect.
- **Docs** — a `<!-- effect: <name> -->` section in `docs/api/Graphics.md` naming each
  tween path, its rows in the sorted function table, and its *Player options* row.
- **Completeness** — `render::postfx::effects::completeness` fails when any layer lacks the
  effect's part, or a composite texture it declares isn't bound and passed at its place in its own function's call. Pixels of every other effect stay byte-identical: no other offset or
  struct moved.

## GPU tests
- **The name says it renders**: in-crate tests are named `gpu_*`, integration tests
  live under `tests/gpu/`. nextest's `gpu` group selects them by name, and a test that
  builds a renderer without such a name panics in debug (`render::setup::gpu_rule`).
- A test reaches a GPU only through `render::test_gpu::headless_or_skip`
  (`make rules`, `test-gpu-entry`); with no adapter it skips. A test that keeps two
  renderers alive at once takes both budget slots together with
  `headless_pair_or_skip` (#1844): a second single one on a thread that holds the
  first panics, since under plain `cargo test` two such tests deadlock.
- **They run on lavapipe** in Linux CI and in cloud sessions (the SessionStart hook
  installs it, #826). `RUSTY_REQUIRE_GPU=1` turns "no adapter" into a failure: set it
  to know they rendered. Metal on macOS is the one real GPU, so pair anything
  load-bearing with an adapter-free unit test of the predicate. Lavapipe has no
  `TIMESTAMP_QUERY`: GPU-timing code is measured only on a real GPU (`make bench`).
- A new GPU test that runs CI out of memory lowers both caps to 1 (the nextest `gpu`
  group and `MAX_CONCURRENT_HEADLESS`) before the test is weakened.

## Pixels
- **Bless by name, and look.** Re-bless a capture or snapshot by its test name, never
  all at once, and open every PNG you bless. A blessing from a container's Mesa may
  differ from what CI renders: say so in the pull request.
- **A pixel change is looked at, before and after** (#1483). The coder renders it on
  `main` and on the branch, opens both, and the pull request says what it saw in a
  `Looked at` line; no PNG goes to GitHub (`docs/testing.md` § *Looking at a
  change*). A refactor that claims no pixel change proves it by comparison.
- No pixel goldens of the editor: fonts and anti-aliasing differ across adapters.
