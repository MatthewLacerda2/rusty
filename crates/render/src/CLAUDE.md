# CLAUDE.md — the renderer

This file loads when you work under `crates/render/src/`. The shaders live in
`engine/shaders/`, which has no CLAUDE.md of its own: **read this file before editing
a `.wgsl` there too.** The root `CLAUDE.md` keeps the project-wide rules;
`docs/testing.md` is the reference for GPU tests.

## Where render sits
- **Platform layer, downstream of the sim.** Render depends on the sim crates its row in
  `tools/lint/src/layers/table.rs` lists; no sim crate depends on `rusty-render`,
  `wgpu` or `egui`, not even in its tests (`make layers`). What the sim needs from shaders (validating a
  bake, say) lives in a sim module render reads, as `shadergen` does, never in a call
  up into render.
- The module root opts out of the clock ban because GPU timing is wall-clock;
  the opt-out stays at the root, never in a file below it.

## The retained scene
- **A camera's mesh solids come from `render::scene`, not a walk of the ECS**. The
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
- **The shadow sweeps gather from the objects too**: a caster's
  cast mode, clips and static-bake terms are kept on its `RenderObject`, and the
  static bakes' key is `RenderScene::statics_key`, refolded only when a static object
  was re-derived. A new input a shadow reads is kept or read per frame the same way,
  and `passes::shadows::statics_key` (the from-scratch digest the tests compare)
  folds it too. A sweep gathers through the scene's BVH of its static objects
  (`scene/casters/`), rebuilt when that key moves: a new path that moves a
  static object's bounds or flags without re-deriving it touches it there
  (`Gathering::touch`, as a pose does), or the sweeps miss it where it went;
  `casters/gather_gpu_tests.rs` and the equivalence tests hold the gather exact.
- **What an object holds lives on the GPU in its slot**: its
  `ObjectData` (world matrix, probe SH, lightmap) sits at its slot of the object table
  (`scene/table.rs`), written where it changed (`gpu/draw_buffers/objects.rs`). A
  camera's `InstanceData` is 16 bytes naming the slot plus what the camera decides
  (reflection probes, proxy volume, motion history in the joint array). A new
  per-object value the shader reads goes into `ObjectData` (Rust and
  `forward/bindings.wgsl`), written in `pose_object`; a per-view one into
  `InstanceData`, and the fragment stage gets it through `VertexOutput::instance`,
  since it is at its 8 storage buffers and never binds the instances.
- **A material's record lives on the GPU in its slot too**: one
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
- **Occlusion culling reads the scene's baked PVS** (`scene/occlusion/`):
  bound per scene and re-checked against a baked object's fingerprint only after
  that static object changed (a whole bind when the PVS, the objects or the
  `LODGroup`s' renderers changed); it may keep what is hidden, and culls what a
  camera sees only through a gap the bake's sampled points miss (about a metre,
  `docs/api/Lighting.md`, *What it can miss*). A new path that decides what a camera draws reads it through
  `Renderer::occlusion_eye`, so a capture, an orthographic camera and a far near
  plane cull nothing. A light's shadow views read it through
  `OcclusionBinding::shadow_row` (the light's cell, culling nothing while a matching
  occluder casts no shadow) and only for a tile whose statics are drawn into it.
  A light inherits the camera's gap caveat, and a still light's cached tile keeps
  a shadow so lost until it re-bakes.
- **Then the GPU culls what is left by depth** (`passes::hiz`): a camera's
  draws are decided in this order: mask, LOD, frustum and size (`scene/draw.rs`),
  the PVS row, then its `occlusion` stage, which draws last frame's visible set's
  depth, builds the Hi-Z pyramid and tests every opaque instance's bounds, leaving
  `SolidResources::culled`: the kept instances and one indirect draw per opaque
  batch. Every pass that draws the opaque batches goes through
  `Renderer::draw_solids` with it (the forward and velocity pass; the depth
  prepass (SSAO's, SSIL's or the priming one) draws only its late set, `Culled::late`, over phase one's kept depth, so a
  new pass that draws depth before the forward pass loads that depth too); the batches are an `OpaqueBatches`, which hands out no slice, so the
  compiler refuses them to `draw_batches` (the translucents' path), which would draw
  what the cull hid. A transparent batch is never culled. `Renderer::hiz_culls` says which camera culls (the preset's
  `gpu_occlusion_culling`, the camera's flag, no capture), and the stage skips one
  testing fewer than `passes::hiz::MIN_INSTANCES` objects and drawing no batch by
  cluster (`Renderer::hiz_draws_enough`), leaving it drawn as with the flag off but still primed
  (`SolidResources::floored`: a prepass of its few batches under the forward pass); an object is tested
  only when `camera_solids` lists it in `CameraSolids::tested`, so a new kind of
  instance (a particle, a ringed object) stays drawn unless it adds itself there.
  A material whose vertices move past the mesh's bounds (`RenderObject::moving`)
  is never listed (`scene/draw.rs`, `hiz_testable`).
  What those indirect draws drew is the GPU's: `Debug.Stats`' `triangles` takes it
  from their args, read back with the stage's counts (`passes::hiz::drawn`), so a
  new pass drawing through `Culled` counts its calls on the CPU and its triangles
  from that read (`RenderCounters::add_read_draws`), the batches' whole count
  standing in only until a read has come back.
- **A big mesh is then culled by cluster** (`passes::hiz::clusters`): the
  import splits a static mesh of at least `asset::clusters::MIN_TRIANGLES` into
  clusters and reorders its indices to match (`MeshComponent::clusters`, valid
  only at the revision it was built for, so an edit drops it), and its GPU mesh
  leases rows of the renderer's one cluster table and a copy of its indices in one
  pool (`GpuMesh::clusters`, `clusters::pool`), so each cluster stage is one
  dispatch for the frame. In phase two a batch of such a
  mesh is drawn per instance from the camera's compacted cluster indices
  (`Culled::clusters`, `ClusterDraws`): `Renderer::draw_solids` does that for
  every opaque pass, so a new opaque pass gets it for free. Phase one still draws
  whole meshes. A cluster test reads the same pyramid through
  `hiz/hidden.wgsl`'s `hidden`, the module both cull shaders import.
- **Rays trace the scene's static geometry through `render::rays`**: a two-level
  tree (one per mesh, shared by its instances, and a top one over the static objects)
  kept on `RenderScene::rays`, rebuilt only after `RayTree::touch` (the same hooks
  that touch the casters) left a static object placed or drawn otherwise, or an active
  camera's culling mask changed, and built the first time `Renderer::ray_tree` is asked.
  A pass tracing every frame takes it with `Renderer::take_rays` before any pass
  records and holds that one until `release_rays` (a top-only upload rewrites the nodes
  a recorded bind group walks); a load's warm-up builds mesh trees ahead
  (`rays::prebuild`) when its volume traces SSR's *Mixed*. A mesh's own tree lives as long as its
  `GpuMesh`, at that mesh's revision. A tree whose buffers would pass the device's
  limits is refused whole (an empty one is bound, `gpu_bytes` 0, said once). A
  shader imports `rt::trace` (`trace_closest`, `trace_any`, `hit_uvs`) and binds `RayTree::bind_group` at `rays::GROUP`; a hit
  names the object's slot, so the rest comes from `ObjectData`, and its run's material
  record slot (`hit_record`, above a face word's cull bits). Only what an opaque
  material draws of a still, static, shown object is in it (`rays::traced`,
  `top::Placed::of`); a new way to leave a static object out goes there, and a new
  path that changes a static object without re-deriving it touches the tree.
  `rays/walk.rs` is the shader's CPU twin, and the tests hold the two equal.

## Shaders
- **Shared WGSL is a naga_oil composable module, never a copy.** WGSL has no
  `#include`. Every shader composes through `shadergen::compose`, the one GPU-free
  composition path, which registers `common.wgsl` as `"common"`; a shader pulls
  it in with `#import "common"`. When two shaders need the same struct or function,
  move it into a module rather than pasting it.
  The fullscreen triangle is one: `#import fullscreen`, and `make rules` flags a copy.
- **Derivatives only in uniform control flow**: `dpdx`, `dpdy`, `fwidth` and an
  implicit-LOD `textureSample` sit before any branch on a per-pixel value, as
  `fold_decals` and `ui.wgsl`'s `shade` do. Inside one, a quad neighbour outside the
  branch leaves them undefined (on Metal they change frame to frame).
  No gate holds it: naga's validator skips the fragment stage's uniformity check, and
  a text match can't tell a per-pixel branch from a uniform one.
- **The forward fragment stage binds all 16 of Metal's samplers** (a hard cap; none
  free), counted from the layouts by `setup::limits`' test. A new engine-owned
  texture in group 0 reads through the shared `s_linear_clamp`
  (`gpu::linear_clamp`), not a sampler of its own; a new material map shares a
  sibling map's sampler, or one is freed first.
- `render::gpu::shaders::ShaderRegistry` reads shaders from disk at runtime, so a
  shader edit needs no Rust rebuild.
- **A mip level that varies from pixel to pixel is read through a sampler**
  (`textureSampleLevel` at the texel's centre), and its size is the first level's
  halved: `textureLoad` and `textureDimensions` at such a level read the wrong level
  on lavapipe while NVIDIA and Metal read the right one. A level uniform across
  the draw may use either.
- wgpu, naga and naga_oil upgrade as a unit (`Cargo.toml`).

## Reuse `render::gpu`
Before building a layout or pipeline by hand, use these:
- `render::gpu::layout_entries`: one uniform, texture, sampler, depth or storage entry.
- `render::gpu::groups`: the solid path's bind-group indices (`rusty_shadergen::groups`,
  by how often each changes, the per-draw group last). A solid-path pipeline layout is
  built through `Groups::layout`, and its `set_bind_group` names `groups::DRAW` and the
  rest, never a number (any other pass names its own `const`; `make rules`,
  `bind-group-index`); `groups`' tests hold the WGSL `@group`s to them.
- `render::gpu::DEPTH_FORMAT`: every view depth target and every pipeline drawing into
  one; shadow maps keep their own format in `passes::shadows`.
- `render::gpu::commands::Commands`: a camera's stages record into its one
  encoder (`CameraCtx::commands`) and never make or submit their own; a read-back
  maps through `Commands::after_send`, and a mid-camera change to a buffer a pass
  already read is a recorded copy (`draw::camera::buffer`) or a `Commands::flush`.
- `render::gpu::kept::Kept`: a bind group or draw kept across frames, rebuilt only
  when a resource its key names was replaced (the Hi-Z cull's, SSR's targets).
- `render::gpu::fullscreen::Fullscreen`: any fullscreen-triangle pass (post-FX, SSAO,
  the UI blur, the shadow-atlas blit).
- `render::passes::blended`: the globals uniform and the alpha and additive pipelines
  into the HDR target that test depth without writing it (particles, ribbons).
- `render::gpu::pipelines::LazyPipeline`: a pipeline a frame may not draw (an
  editor overlay, a blend mode, a pass only some scenes use) is built on its first
  draw, not with the renderer: on lavapipe each pipeline is an LLVM compile, and every
  GPU test builds its own renderer. A lazy pipeline must be listed in
  `Renderer::lazy_pipelines` (`render::setup::warm`), so `warm_pipelines` builds it
  when a windowed renderer starts: a game never compiles one mid-frame.

## Adding a post effect
A post effect is a module in every layer: it adds its own files and **one
row per registry** (each folder's `mod` list counts as one), and edits no shared
struct, shader body or list position. Group a family (`grading/`, `lens/`) the same way
in every layer. The vignette, the smallest real effect, is the worked example: grep
`vignette` for every file and row.
- **Settings** — `components/rendering/post_processing/<effect>.rs` (serde, `Default`,
  the clamp constants) and one field on `PostProcessingComponent`, plus its line in
  that struct's `Default` (the compiler asks for it: the volume applies by default, so
  the impl is written out). A camera effect keeps its knobs on `CameraComponent`,
  named `<effect>_…`.
- **Ops** — `edit/authoring/rendering/post_processing/<effect>.rs`: its ops, its
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
  TAA upscales the view (TAAU: a render scale below 1 with TAA on), when the
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
  function (`effects::textures`), so no effect edits `postfx/frame.wgsl`, the
  io layout or `PostFx::run`. A `view` of `None` binds one shared black texel, so the
  function must draw nothing when it reads black (bloom's dirt, the lens flare), and
  an effect that is off skips the fetch on a slot's on flag, as bloom and the flare
  do (`p.w`), so per pixel it costs a uniform branch, not a texture read. It samples with the chain's own samplers (`s_color`, `s_skybox`).
- **Gate** — its `PostEffect::gate`: the player option, when it is player-facing.
  It asks through `EffectCx::runs(EFFECT.gate, on)`; no shared `match` names it. No
  quality preset gates an effect.
- **History** — its `PostEffect::history`: `History::Reads(runs)` when it keeps
  per-view state a later frame reads (TAA, SSIL, the fog), else
  `History::None("<why>")`; the completeness gate requires one.
- **Quality** — an effect with a cost knob carries an `EffectQuality` selector and its
  `Custom` values on its settings, and its row in `core::settings::quality::QualityValues`
  (three `Levels`); its render code picks with `quality.pick(&graphics.quality.<row>, custom)`.
- **Option** — a player-facing effect adds its `GraphicsOption` row, named as the
  effect.
- **Docs** — a `<!-- effect: <name> -->` section in `docs/api/Graphics.md` naming each
  tween path, its rows in the sorted function table, and its *Player options* row.
- **Completeness** — `render::postfx::effects::completeness` fails when any layer lacks the
  effect's part, or a composite texture it declares isn't bound and passed at its place in its own function's call. Pixels of every other effect stay byte-identical: no other offset or
  struct moved. Three layers sit above this crate's live code: the authoring layer
  (the field tables, `EFFECTS`, the camera's knob tables), handed in as an `Authored`,
  and the `Graphics.*` registrars and the Inspector's sections. The gate's own cases
  (`tests/post_effect_cases/`) fill those two with the renderer's effect names and so
  cannot see either one missing: `tests/post_effect_layers.rs` hands the real ones in,
  and is what catches a missing registrar or Inspector section.

## GPU tests
- **The name says it renders**: in-crate tests are named `gpu_*`, integration tests
  live under `tests/gpu/`. nextest's GPU group (`gpu`, `gpu-macos` on macOS) selects them by name (a `gpu::`
  path only in the integration binary: this crate's `gpu` module is no GPU mark), and a test that
  builds a renderer without such a name panics in debug (`render::setup::gpu_rule`).
- A test reaches a GPU only through `render::test_gpu::headless_or_skip`
  (`make rules`, `test-gpu-entry`); with no adapter it skips. A test that keeps two
  renderers alive at once takes both budget slots together with
  `headless_pair_or_skip`: a second single one on a thread that holds the
  first panics, since under plain `cargo test` two such tests deadlock.
- **They run on lavapipe** in Linux CI and in cloud sessions (the SessionStart hook
  installs it). `RUSTY_REQUIRE_GPU=1` turns "no adapter" into a failure: set it
  to know they rendered. Metal on macOS is CI's one real GPU, so pair anything
  load-bearing with an adapter-free unit test of the predicate. Lavapipe has no
  `TIMESTAMP_QUERY`: GPU-timing code is measured only on a real GPU (`make bench`).
- **A tolerance holds on lavapipe, Metal and a real desktop GPU**, and says beside it
  what it was measured on; one added or moved runs on a desktop GPU before it merges
  (`make test-gpu ADAPTER=<gpu>`, `docs/testing.md`, *Which adapters a GPU tolerance
  holds on*).
- A new GPU test that runs CI out of memory lowers both caps to 1 on its platform (the
  nextest `gpu` or `gpu-macos` group and `MAX_CONCURRENT_HEADLESS`) before the test is weakened.

## Pixels
- **Bless by name, and look.** Re-bless a capture or snapshot by its test name, never
  all at once, and open every PNG you bless. A blessing from a container's Mesa may
  differ from what CI renders: say so in the pull request.
- **A pixel change is looked at, before and after**. The coder renders it on
  `main` and on the branch, opens both, and the pull request says what it saw in a
  `Looked at` line; no PNG goes to GitHub (`docs/testing.md` § *Looking at a
  change*). A refactor that claims no pixel change proves it by comparison.
- No pixel goldens of the editor: fonts and anti-aliasing differ across adapters.
