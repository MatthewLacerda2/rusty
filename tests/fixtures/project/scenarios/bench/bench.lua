-- The stress run behind `make bench`, on the default yard with the shared HUD. Measures FRAMES
-- frames at 1280x720 (quiet), then runs `firefight.lua` until its live counts hold still and
-- measures FRAMES more (`firefight.<row>`), each against the last run (out/bench/bench.json).
-- Fixed dt and seeded math.random; timings are this machine's, only counts compare. A signal.
local HERE = "scenarios/bench/"
local WARMUP = 60    -- 1 s: scripts start, agents set off, shadow caches bake
local FRAMES = 600   -- 10 s @ 1/60, a window
-- Ticks past firefight.lua's LIFETIME and DECALS, so its live counts hold still before the window.
local FIREFIGHT_WARMUP = 120

local spawned = Harness.LoadStress{
    enemy_script = HERE .. "enemy.lua",
    flicker_script = HERE .. "flicker.lua",
}
Harness.Expect(spawned.enemies == 50, "50 soldiers spawned")
Harness.Expect(spawned.lights == 32, "32 realtime lights spawned")
Harness.Expect(spawned.decals == 256, "256 decals stamped")

local hud = Scene.CreateEntity("HUD")
Scene.AddScript(hud, HERE .. "hud.lua")
Scene.AddScript(hud, HERE .. "hud_cameras.lua")
Scene.AddScript(Scene.CreateEntity("HUD_Feed"), HERE .. "hud_feed.lua")

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
