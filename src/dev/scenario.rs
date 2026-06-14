//! src/dev/scenario.rs — Scenario loader / runner
//!
//! A scenario is a normal `.lua` file (tagged dev-only) that uses the SAME engine
//! API as gameplay, plus the Harness/Debug dev API. The agent loop is:
//!
//!   1. write   project/scenarios/<name>.lua
//!   2. run     the headless harness on it
//!   3. read    results.json + console.log + *.png   (PNGs are viewable directly)
//!
//! Example scenario:
//!   local enemy = Scene.FindEntityByName("Enemy_1")
//!   Input.Press("W"); Harness.Step(120)            -- walk 2s @ 1/60
//!   Debug.Screenshot("after_walk.png")
//!   Harness.Shoot(); Harness.Step(1)
//!   Harness.Expect(Health.Get(enemy) < 100, "hitscan should damage enemy")
//!
//! Allowed deps: dev::harness, dev::console, api.
//! Status: SCAFFOLD — structure only; not yet implemented.
