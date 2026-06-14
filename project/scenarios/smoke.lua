-- project/scenarios/smoke.lua — the canonical headless smoke test.
--
-- Run it with:
--   cargo run --bin play --features dev -- project/scenarios/smoke.lua out/
--
-- It drives the demo world the editor shows (Player = 2, Enemy_1 = 5) entirely
-- headlessly: press a key, skip a couple seconds of ticks with one StepUntil, then
-- observe ONCE and assert. Deterministic — rerun it and results.json is identical.

local player = Scene.FindEntityByName("Player")
local enemy = Scene.FindEntityByName("Enemy_1")

Harness.Expect(player ~= nil, "demo scene has a Player entity")
Harness.Expect(enemy ~= nil, "demo scene has an Enemy_1 entity")

-- Record the starting position so we can prove the sim actually moved the player.
local sx, sy, sz = Transform.GetPosition(player)
Harness.Log(string.format("player start: %.2f, %.2f, %.2f", sx, sy, sz))

-- Walk forward for ~2 seconds (120 ticks @ 1/60). Player should leave the spot.
Input.Press("W")
Harness.Step(120)
Input.Release("W")

local ex, ey, ez = Transform.GetPosition(player)
Harness.Log(string.format("player end:   %.2f, %.2f, %.2f", ex, ey, ez))

local moved = math.abs(ex - sx) + math.abs(ez - sz)
Harness.Expect(moved > 0.1, "holding W moves the player")
Harness.Expect(Harness.Frame() == 120, "exactly 120 ticks were simulated")

-- StepUntil: skip up to 120 more ticks; the enemy keeps its Walk clip the whole time.
local stopped = Harness.StepUntil(function()
    return Animator.GetClip(enemy) ~= "Walk"
end, 120)
Harness.Expect(stopped == false, "enemy stays in Walk for the observation window")

Harness.Log("smoke scenario complete")
