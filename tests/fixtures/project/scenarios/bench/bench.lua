-- Shooter-shaped stress run behind `make bench`: 50 skinned soldiers with Lua brains, 32 realtime
-- lights, 256 decals and four smoke columns on the default yard. Measures FRAMES frames at 1280x720
-- (quiet), then runs `firefight.lua` until its live counts hold still and measures FRAMES more
-- (`firefight.<row>`), each against the last run (out/bench/bench.json). Fixed dt and seeded
-- math.random; timings are this machine's, only counts compare across machines. A signal, never a
-- gate.
local HERE = "scenarios/bench/"
local WARMUP = 60
local FRAMES = 600   -- 10 s @ 1/60, a window
local FIREFIGHT_WARMUP = 120

local spawned = Harness.LoadStress{
    enemy_script = HERE .. "enemy.lua",
    flicker_script = HERE .. "flicker.lua",
}
Harness.Expect(spawned.enemies == 50, "50 soldiers spawned")
Harness.Expect(spawned.lights == 32, "32 realtime lights spawned")
Harness.Expect(spawned.decals == 256, "256 decals stamped")

-- Warm up rendering too: first-use costs stay out of the measured frames.
for _ = 1, WARMUP do
    Harness.Step(1)
    Harness.Render()
end

local report = Harness.Bench(FRAMES)
Harness.Expect(report.frames == FRAMES, "every frame was measured")

local firefight = Scene.CreateEntity("Firefight")
Scene.AddScript(firefight, HERE .. "firefight.lua", { prefab = HERE .. "casing.prefab" })
for _ = 1, FIREFIGHT_WARMUP do
    Harness.Step(1)
    Harness.Render()
end
local fight = Harness.Bench(FRAMES, "firefight")
Harness.Expect(fight.frames == FRAMES, "every firefight frame was measured")
