-- scenarios/load.lua — the showroom loads from disk with everything the recipe
-- built in it, and plays a few seconds.

local names = { "Floor", "Mirror", "WindowPane", "TintedPane", "Glass", "Sign",
    "Pedestal", "Ball", "Sun", "Camera" }
for _, name in ipairs(names) do
    Harness.Expect(Scene.FindEntityByName(name) ~= nil, "the showroom has " .. name)
end

-- What the room is for survives the save and load: the glass is transparent and
-- refracts, the probes are there, and the floor is in SSR's reach.
for _, name in ipairs({ "WindowPane", "TintedPane", "Glass" }) do
    local id = Scene.FindEntityByName(name)
    Harness.Expect(Material.GetIndexOfRefraction(id) == 1.5, name .. " has ior 1.5")
end
Harness.Expect(Probe.Count() == 6, "six light probes along the ball's path")
Harness.Expect(Reflection.Count() == 2, "a box and a sphere reflection probe")
Harness.Expect(Reflection.Get(0).shape == "Box" and math.abs(Reflection.Get(0).rotation.y - 15) < 1e-3,
    "the west probe is a box turned 15 degrees")
Harness.Expect(Reflection.Get(1).shape == "Sphere", "the east probe is a sphere")
local smoothness = Graphics.GetSsrMinSmoothness()
Harness.Expect(Graphics.GetSsrActive() and math.abs(smoothness - 0.1) < 1e-6,
    "SSR runs from a minimum smoothness of 0.1 (got " .. smoothness .. ")")

Harness.Step(180)
local _, y = Transform.GetPosition(Scene.FindEntityByName("Ball"))
Harness.Expect(math.abs(y - 0.3) < 1e-4, "the ball stays on the floor while it rolls")
