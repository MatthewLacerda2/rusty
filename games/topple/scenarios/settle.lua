-- scenarios/settle.lua — every level, left alone, stays standing: between half
-- a second in (the stacks have taken their weight) and three seconds in, no
-- body moves more than TOLERANCE, and every target is still on its platform.
-- A tower that jitters, creeps or slowly slides fails it.

local TOLERANCE = 0.01 -- metres

local function bodies(id, into)
    for _, child in ipairs(Scene.GetChildren(id)) do
        if Component.Get(child, "RigidBody") then into[#into + 1] = child end
        bodies(child, into)
    end
    return into
end

local function poses(list)
    local out = {}
    for i, id in ipairs(list) do out[i] = { Transform.GetPosition(id) } end
    return out
end

Harness.Step(1) -- enter Play
for n = 1, 4 do
    Scene.Load(string.format("assets/scenes/level_%d.scene", n))
    Harness.Step(30) -- the load lands, the scripts start, the stacks take their weight
    local list = bodies(Scene.FindEntityByName("Bodies"), {})
    local before = poses(list)
    Harness.Step(150)
    local after = poses(list)
    local worst, name = 0, "-"
    for i = 1, #list do
        local a, b = before[i], after[i]
        local d = math.sqrt((a[1] - b[1]) ^ 2 + (a[2] - b[2]) ^ 2 + (a[3] - b[3]) ^ 2)
        if d > worst then worst, name = d, Scene.GetName(list[i]) end
    end
    if n == 4 then Harness.Screenshot("cache/test/shots/pile.png", { settle = 0 }) end
    local level = Scene.GetScript(Scene.FindEntityByName("Level"), "level")
    local standing, total = level.Standing(), #level.targets
    Harness.Log(string.format("level %d: %d bodies, worst drift %.5f m (%s), %d/%d targets standing",
        n, #list, worst, name, standing, total))
    Harness.Expect(#list > 0, string.format("level %d has bodies", n))
    Harness.Expect(worst < TOLERANCE, string.format("level %d: no body drifts %.0f cm in 2.5 s", n, TOLERANCE * 100))
    Harness.Expect(total > 0 and standing == total, string.format("level %d: every target still stands", n))
end
