-- scenarios/replay.lua — the same throws on the same seed, twice, end with
-- every body of level 2 (the plank tower and the jointed figure) in the same
-- place and the same orientation, to the bit.

local function bodies(id, into)
    for _, child in ipairs(Scene.GetChildren(id)) do
        if Component.Get(child, "RigidBody") then into[#into + 1] = child end
        bodies(child, into)
    end
    return into
end

-- Load level 2, throw at the figure and then the tower, let it all land, and
-- return every body's name, position and rotation.
local function run(shot)
    Scene.Load("assets/scenes/level_2.scene")
    Harness.Step(30)
    Random.SetSeed(7)
    local thrower = Scene.GetScript(Scene.FindEntityByName("Player"), "thrower")
    thrower.ThrowAt(1.2, 2.1, 0)
    Harness.Step(42)
    -- The figure mid-fall, seen from the side. Both runs move the camera and
    -- put it back, so they stay alike; only the first takes the picture.
    local x, y, z = Camera.GetPosition()
    local yaw, pitch = Camera.GetYaw(), Camera.GetPitch()
    Camera.SetPosition(7, 2.6, 1)
    Camera.SetYaw(180)
    Camera.SetPitch(-8)
    Camera.ResetHistory()
    if shot then Harness.Screenshot("cache/test/shots/figure_falling.png", { settle = 2 }) end
    Camera.SetPosition(x, y, z)
    Camera.SetYaw(yaw)
    Camera.SetPitch(pitch)
    Camera.ResetHistory()
    thrower.ThrowAt(-1.2, 1.6, 0)
    Harness.Step(200)
    local out = {}
    for i, id in ipairs(bodies(Scene.FindEntityByName("Bodies"), {})) do
        local x, y, z = Transform.GetPosition(id)
        local rx, ry, rz = Transform.GetRotation(id)
        out[i] = { Scene.GetName(id), x, y, z, rx, ry, rz }
    end
    return out
end

Harness.Step(1)
local first = run(true)
local second = run()
Harness.Expect(#first == #second and #first > 0, "both runs have the same bodies")
local worst, moved = 0, 0
for i = 1, #first do
    local a, b = first[i], second[i]
    Harness.Expect(a[1] == b[1], "body " .. i .. " is the same body in both runs")
    for k = 2, 7 do worst = math.max(worst, math.abs(a[k] - b[k])) end
    if a[1] == "TargetTorso" then moved = 2.05 - a[3] end
end
Harness.Log(string.format("%d bodies, largest difference between the runs %.3g", #first, worst))
Harness.Expect(moved > 0.5, "the figure was knocked down (its torso dropped)")
Harness.Expect(worst == 0, "the two runs end bit for bit the same")
