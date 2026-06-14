# Architecture: the conceptual model

A stable, high-level map of *how the engine is shaped* — the kinds of moving parts
and how they fit together. It deliberately does **not** enumerate every concrete
type or track per-item status; that inventory drifts. For the live, exact list:

- **Engine types & systems** → the rustdoc reference (`cargo doc --no-deps`,
  published to GitHub Pages) — generated from the `///` comments on the real code.
- **Script API surface** → [`docs/scripting-api.md`](docs/scripting-api.md).
- **Module tree & the Unity-shaped / agentic story** → [`README.md`](README.md).

---

## The five kinds of moving part

Everything in the engine is one of these. Unity analogs in parentheses.

1. **Resources** — engine singletons, one per World (Unity's engine statics:
   `Time`, `Input`, the nav graph, the console, the active camera, play-state, the
   renderer). Global state the systems read and write.

2. **Components** — per-entity data, the Unity-style "classes" (`Transform`,
   `Mesh`, `Camera`, `Light`, `Collider`, `Rigidbody`, `NavMeshAgent`, `Health`,
   `Animator`, …). First-class and engine-provided; systems expect them. **Every
   entity has exactly one `Transform` (mandatory, cannot be removed); all other
   components are optional.** Custom behaviour goes in *scripts*, never in new
   built-in components.

3. **Systems** — per-frame logic, a plain `fn(&mut World, &mut Resources)`, grouped
   into ordered stages (`Startup` once, then each frame
   `FixedUpdate → Update → LateUpdate → Render`). Order within a stage is the order
   modules `register` them. `FixedUpdate` is the deterministic, fixed-dt stage the
   headless harness steps.

4. **Scene & serialization** — one active scene as a serde `SceneData` document
   (references + values, no GPU buffers). Save/load replaces the World; a
   clone-on-Play / restore-on-Stop snapshot makes edit-mode authoritative, mirroring
   Unity's play-mode behaviour.

5. **The API surface** — one stable set of namespaces (`Transform`, `Input`,
   `Time`, `Physics`, `Scene`, `Animator`, `Nav`, `Health`, `Camera`, `Material`,
   and the dev-only `Debug`) shared by gameplay scripts, the console REPL, and
   bot-players. One surface, three callers — they can never drift apart.

---

## The invariants that hold it together

- **The simulation knows nothing about rendering.** That separation is what makes
  headless, deterministic play possible — the harness steps the sim with no GPU.
- **Determinism.** The sim is a pure function of (seed, inputs, fixed dt). Wall-clock
  reads and unseeded RNG are banned from the sim modules (`app`, `scripting`,
  `physics`, `navigation`) and live only in the platform layer (`main.rs`, `render`,
  `dev`). Enforced by the determinism guard in `tools/lint`.
- **No event bus, no plugin trait.** Cross-system signals are direct typed returns;
  modules wire up via a plain `register(&mut app)` fn.
- **Dev-only layer compiles out.** The console/REPL, harness, bot-players and
  `Debug.*` are `#[cfg(feature = "dev")]` and stripped from ship builds.

---

## Status

The engine currently runs from the legacy modules (`core/`, `render/`, `physics/`,
`scripting/`, `navigation/`, `editor/`); the `app/`, `ecs/`, `scene/`, `api/`,
`dev/` trees are the scaffold being migrated into. See the README's *Migration*
section for the target shape.
