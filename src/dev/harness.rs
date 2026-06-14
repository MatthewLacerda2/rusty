//! src/dev/harness.rs — Headless deterministic runner (the agent's hands)
//!
//! Builds an App with NO render systems, runs the fixed-timestep clock as fast as
//! the CPU allows, and exposes the control surface an agent needs:
//!
//!   Step(n)              advance exactly n fixed ticks (default dt = 1/60).
//!   StepUntil(pred, max) advance until a Lua predicate is true or max ticks pass.
//!                        This is the "skip ticks" primitive: run a whole match,
//!                        observe ONCE — spend one observation instead of hundreds.
//!   Snapshot()           dump world state (entities: id/name/pos/rot/health/clip,
//!                        camera, play-state) as JSON.
//!   Log / Expect         record observations + pass/fail assertions.
//!
//! Determinism contract: fixed timestep + frame-count-based logic only (no wall-clock
//! Instant reads inside the tick), so a scenario replays identically every run. That
//! is what makes an agent-reported bug reproducible by you.
//!
//! A run produces: <out_dir>/results.json + <out_dir>/console.log + screenshots.
//!
//! Allowed deps: app, ecs, scripting, api. Renderer ONLY via dev::screenshot.
//! Status: SCAFFOLD — structure only; not yet implemented.
