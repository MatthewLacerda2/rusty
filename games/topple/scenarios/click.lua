-- The first level, played with the mouse: the cursor is
-- free and shown, a click on the tower's target throws at it through the input
-- path (`Camera.MouseRay`), and the target falls off the platform.

Harness.Step(1) -- enter Play: the scripts start
local level = Scene.GetScript(Scene.FindEntityByName("Level"), "level")
local thrower = Scene.GetScript(Scene.FindEntityByName("Player"), "thrower")
Harness.Expect(not Input.IsCursorLocked() and Input.IsCursorVisible(), "the cursor is free and shown")

-- The pointer over the red box on top of the tower. `WorldToScreen` is
-- bottom-left, `MoveMouse` top-left: the one flip a click needs by hand.
local _, h = UI.GetScreenSize()
local sx, sy, _, on = Camera.WorldToScreen(0, 3.25, 0)
Harness.Expect(on, "the target is on screen")
Input.MoveMouse(sx, h - sy)
local ox, oy, oz, dx, dy, dz = Camera.MouseRay()
local _, hit = Physics.Raycast(ox, oy, oz, dx, dy, dz, nil, nil, 60)
Harness.Expect(hit == Scene.FindEntityByName("Target"), "the ray under the cursor hits the target")

Input.Press("MOUSE0")
Harness.Step(1)
Input.Release("MOUSE0")
Harness.Expect(thrower.left == 4, "the click threw one ball, " .. thrower.left .. " left")
Harness.Step(240)
Harness.Expect(level.Standing() == 0, "the tower's target fell off the platform")
