-- scenarios/ball.lua — the ball rolls from the dark west half into the sunlit
-- east one, and the probe lighting it takes changes as it goes.

local ball = Scene.FindEntityByName("Ball")
Harness.Expect(ball ~= nil, "the showroom has a Ball")

-- What the probe field gives the ball's centre, averaged over the six axis
-- directions: its probe lighting. The probes hold the light bounced into the
-- room (the sun reaches the ball directly, not through them), so a ball over the
-- sunlit patch is lit from below by it.
local AXES = { { 1, 0, 0 }, { -1, 0, 0 }, { 0, 1, 0 }, { 0, -1, 0 }, { 0, 0, 1 }, { 0, 0, -1 } }
local function light()
    local x, y, z = Transform.GetPosition(ball)
    local sum = 0
    for _, n in ipairs(AXES) do
        local r, g, b = Probe.SampleIrradiance(x, y, z, n[1], n[2], n[3])
        sum = sum + 0.2126 * r + 0.7152 * g + 0.0722 * b
    end
    return x, sum / #AXES
end

-- The ball at its start, in the dark half (cache/test/shots/ball_dark.png, where
-- the check holds it to having an image). Taken here rather than in `shots.lua`,
-- whose shots settle eight frames each: the showroom runs no TAA, so a settle
-- changes no pixel and costs eight renders (#2090).
Camera.SetPosition(-2.0, 1.2, -3.6)
Camera.SetYaw(math.deg(math.atan(-1.0 + 3.6, -5.0 + 2.0)))
Camera.SetPitch(math.deg(math.atan(0.3 - 1.2, math.sqrt(3.0 ^ 2 + 2.6 ^ 2))))
Camera.ResetHistory()
if not Harness.Screenshot("cache/test/shots/ball_dark.png") then
    Harness.Log("shot skipped (no GPU adapter): ball_dark")
end

local x0, l0 = light()
if l0 == 0 and Reflection.Get(0).cubemap == "" then
    -- Generated with no GPU or software adapter: the probes were never baked.
    Harness.Log("skipped: the probes are unbaked (no GPU adapter at generation)")
    return
end
Harness.Step(480)
local x1, l1 = light()
Harness.Log(string.format("ball x %.2f -> %.2f, probe light %.4f -> %.4f", x0, x1, l0, l1))
Harness.Expect(x1 - x0 > 8, "roll.lua carries the ball across the room")
Harness.Expect(l1 > 2 * l0, "the ball is lit brighter in the sunlit half than in the dark one")
local _, _, turned = Transform.GetRotation(ball)
Harness.Expect(math.abs(turned) > 1, "the ball turns as it rolls")

-- The ball in the sunlit half, for looking (cache/test/ball/bright.png).
Camera.SetPosition(1.0, 1.2, -3.6)
Camera.SetYaw(math.deg(math.atan(-1.0 + 3.6, 3.5 - 1.0)))
Camera.SetPitch(-15)
Camera.ResetHistory()
if not Harness.Screenshot("cache/test/ball/bright.png") then
    Harness.Log("shot skipped (no GPU adapter): bright")
end
