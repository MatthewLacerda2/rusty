-- The city's layout: the streets are walkable at their heights, the buildings
-- stand, and Play draws from the street camera alone.

Harness.Step(2) -- enter Play: the viewpoints' Start runs

local function count(id)
    local n = 1
    for _, child in ipairs(Scene.GetChildren(id)) do n = n + count(child) end
    return n
end
local city = Scene.FindEntityByName("City")
Harness.Expect(city ~= nil, "the city scene has its City root")
Harness.Log("entities under City: " .. count(city))
Harness.Expect(count(city) == 3000, "the default city holds exactly objects = 3000 entities")

local cameras = Scene.GetChildren(Scene.FindEntityByName("Viewpoints"))
Harness.Expect(#cameras >= 6 and #cameras <= 8, "the city authors six to eight viewpoints, as the bench takes")
local live = {}
for _, id in ipairs(cameras) do
    if Scene.IsActiveInHierarchy(id) then live[#live + 1] = Scene.GetName(id) end
end
Harness.Expect(#live == 1 and live[1] == "Street", "Play draws from the Street camera alone: "
    .. table.concat(live, ", "))

-- Rays down: on an avenue's asphalt, its sidewalk, and a building's roof.
local function ground(x, z)
    local hit, _, _, _, y = Physics.Raycast(x, 500, z, 0, -1, 0)
    return hit and y or nil
end
local x, _, z = Transform.GetPosition(Scene.FindEntityByName("Avenue"))
local asphalt, walk = ground(x, z), ground(x + 10, z)
Harness.Log(string.format("avenue at x %.1f: asphalt %s, sidewalk %s", x, tostring(asphalt), tostring(walk)))
Harness.Expect(asphalt and math.abs(asphalt) < 0.01, "an avenue's asphalt is at the ground")
Harness.Expect(walk and math.abs(walk - 0.15) < 0.01, "its sidewalk is a curb above it")

-- A lamp head is draw-only: a ray down through it lands on the sidewalk below.
local lx, ly, lz = Transform.GetPosition(Scene.FindEntityByName("Lamp"))
local under = ground(lx, lz)
Harness.Log(string.format("lamp head at y %.2f: ray down lands at %s", ly, tostring(under)))
Harness.Expect(under and math.abs(under - 0.15) < 0.01, "a lamp head carries no collider")

local tallest = 0
for _, id in ipairs(Scene.GetChildren(Scene.FindEntityByName("Buildings"))) do
    local _, sy = Transform.GetLocalScale(id)
    tallest = math.max(tallest, sy)
end
Harness.Expect(tallest > 20, "the core has buildings taller than 20 m: " .. tallest)

local warnings = Harness.Warnings()
Harness.Expect(#warnings == 0, "the city run logs no warnings: "
    .. (warnings[1] and warnings[1].text or "none"))
