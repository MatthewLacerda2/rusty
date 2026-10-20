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
- The module root opts out of the clock ban because GPU timing and shader hot-reload
  are wall-clock (#757); the opt-out stays at the root, never in a file below it.

## Shaders
- **Shared WGSL is a naga_oil composable module, never a copy.** WGSL has no
  `#include`. Every shader composes through `shadergen::compose`, the one GPU-free
  composition path (#722), which registers `common.wgsl` as `"common"`; a shader pulls
  it in with `#import "common"`. When two shaders need the same struct or function,
  move it into a module rather than pasting it (#1063 splits the forward shader this
  way).
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

## GPU tests
- **The name says it renders**: in-crate tests are named `gpu_*`, integration tests
  live under `tests/gpu/`. nextest's `gpu` group selects them by name, and a test that
  builds a renderer without such a name panics in debug (`render::setup::gpu_rule`).
- A test reaches a GPU only through `render::test_gpu::headless_or_skip`
  (`make rules`, `test-gpu-entry`); with no adapter it skips.
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
- **A pixel change is stated with before and after.** The pull request shows the
  capture on `main` and on the branch (`make editor-capture` for the editor, as
  `docs/testing.md` § *Editor captures* says), and no PNG stays on the branch that
  merges. A refactor that claims no pixel change proves it by comparison.
- No pixel goldens of the editor: fonts and anti-aliasing differ across adapters.
