-- project/scenarios/bench/bench.lua — the shooter-shaped stress run behind `make bench` (#835).
--
-- Run it with:
--   make bench
--   (= cargo run --release --features dev --bin play -- project/scenarios/bench/bench.lua out/bench)
--
-- Loads the worst case a shooter level asks of the engine onto the default yard
-- (fy_pool_day): 50 skinned, animated soldiers walking the navmesh with a Lua brain
-- each, 32 realtime lights (8 flickering like muzzle flashes, 6 casting shadows),
-- a full decal registry and four smoke columns. Then it warms up and
-- measures FRAMES frames, each stepped and rendered at 1280x720, and prints the
-- report with its change against the last run (kept in out/bench/bench.json).
--
-- Deterministic: fixed dt, seeded math.random, no randomness in placement — two
-- runs on one machine measure the same frames. The timings are this machine's;
-- only the counts compare across machines. A signal, never a gate.

local HERE = "project/scenarios/bench/"
local WARMUP = 60    -- 1 s: scripts start, agents set off, shadow caches bake
local FRAMES = 600   -- 10 s @ 1/60

local spawned = Harness.LoadStress{
    enemy_script = HERE .. "enemy.lua",
    flicker_script = HERE .. "flicker.lua",
}
Harness.Expect(spawned.enemies == 50, "50 soldiers spawned")
Harness.Expect(spawned.lights == 32, "32 realtime lights spawned")
Harness.Expect(spawned.decals == 256, "the decal registry is full")

-- Warm up rendering too, so first-use costs (pipelines, uploads, the static
-- shadow bake) stay out of the measured frames.
for _ = 1, WARMUP do
    Harness.Step(1)
    Harness.Render()
end

local report = Harness.Bench(FRAMES)
Harness.Expect(report.frames == FRAMES, "every frame was measured")
