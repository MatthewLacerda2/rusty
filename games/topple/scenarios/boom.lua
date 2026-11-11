-- scenarios/boom.lua — a ball into the barrel of level 3 sets it off: every
-- body within the blast's reach is knocked away, every body outside it stays
-- where it was. The far stack stands 6 m from the barrel, the reach is 3.5 m.

local REACH = 3.5

local function bodies(id, into)
    for _, child in ipairs(Scene.GetChildren(id)) do
        if Component.Get(child, "RigidBody") and Scene.GetName(child) ~= "Barrel" then
            into[#into + 1] = child
        end
        bodies(child, into)
    end
    return into
end

Harness.Step(1)
Scene.Load("assets/scenes/level_3.scene")
Harness.Step(30)
local barrel = Scene.FindEntityByName("Barrel")
local bx, by, bz = Transform.GetPosition(barrel)
local list = bodies(Scene.FindEntityByName("Bodies"), {})
local start = {}
for i, id in ipairs(list) do start[i] = { Transform.GetPosition(id) } end

local thrower = Scene.GetScript(Scene.FindEntityByName("Player"), "thrower")
thrower.ThrowAt(bx, by, bz)
local ticks = 0
while Scene.FindEntityByName("Barrel") and ticks < 120 do
    Harness.Step(1)
    ticks = ticks + 1
end
Harness.Expect(Scene.FindEntityByName("Barrel") == nil, "the ball sets the barrel off")
Harness.Log(string.format("the barrel went off %d ticks after the throw", ticks))
Harness.Step(4)
Harness.Screenshot("cache/test/shots/boom.png", { settle = 0 })
Harness.Step(116)

local inside, outside = 0, 0
for i, id in ipairs(list) do
    local s = start[i]
    local x, y, z = Transform.GetPosition(id)
    local moved = math.sqrt((x - s[1]) ^ 2 + (y - s[2]) ^ 2 + (z - s[3]) ^ 2)
    local away = math.sqrt((s[1] - bx) ^ 2 + (s[3] - bz) ^ 2)
    local name = Scene.GetName(id)
    if away < REACH - 1 then
        inside = inside + 1
        Harness.Expect(moved > 0.3, string.format("%s %.1f m from the barrel is knocked away (moved %.2f m)", name, away, moved))
    elseif away > REACH + 1 then
        outside = outside + 1
        Harness.Expect(moved < 0.01, string.format("%s %.1f m from the barrel stays put (moved %.4f m)", name, away, moved))
    end
end
Harness.Log(string.format("%d bodies inside the reach, %d outside", inside, outside))
Harness.Expect(inside >= 10 and outside >= 3, "the level has bodies on both sides of the reach")
