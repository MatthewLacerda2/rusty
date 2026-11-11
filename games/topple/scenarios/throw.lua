-- scenarios/throw.lua — the first level, played: one ball thrown at the tower
-- topples it, its target falls off the platform, the level counts it cleared,
-- and the second level loads.

Harness.Step(1) -- enter Play: the scripts start
local level = Scene.GetScript(Scene.FindEntityByName("Level"), "level")
local thrower = Scene.GetScript(Scene.FindEntityByName("Player"), "thrower")
Harness.Expect(level.Standing() == 1 and not level.cleared, "level 1 starts with its target standing")

-- At the red box on top of the tower.
local ball = thrower.ThrowAt(0, 3.25, 0)
Harness.Expect(ball ~= nil, "the thrower has a ball")
Harness.Step(240)
Harness.Log(string.format("after 4 s: %d target(s) standing, %d ball(s) left", level.Standing(), thrower.left))
Harness.Expect(thrower.left == 4, "one ball was thrown")
Harness.Expect(level.Standing() == 0, "the tower's target fell off the platform")
Harness.Expect(level.cleared, "the level counts itself cleared")

Harness.Step(150) -- past level.lua's wait
Harness.Expect(Scene.GetActivePath() == "assets/scenes/level_2.scene", "the second level loaded")
