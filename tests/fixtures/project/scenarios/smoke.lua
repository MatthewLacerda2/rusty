-- The canonical headless smoke test over the demo world (Player = 2, Enemy_1 = 5): press a key,
-- skip ahead with one StepUntil, observe once and assert. Deterministic.

local player = Scene.FindEntityByName("Player")
local enemy = Scene.FindEntityByName("Enemy_1")

Harness.Expect(player ~= nil, "demo scene has a Player entity")
Harness.Expect(enemy ~= nil, "demo scene has an Enemy_1 entity")

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

-- The enemy's animator is script-driven the whole window: its clip is always one of its real clips.
local broke = Harness.StepUntil(function()
    local clip = Animator.GetClip(enemy)
    return clip ~= "Walk" and clip ~= "Idle"
end, 120)
Harness.Expect(broke == false, "enemy animator stays script-driven (Walk/Idle)")
Harness.Log(string.format("enemy clip at end: %s", Animator.GetClip(enemy)))

Harness.Log("smoke scenario complete")
