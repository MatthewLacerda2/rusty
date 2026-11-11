-- scenarios/moves.lua — the smoke game's crate is moved by its script.

local crate = Scene.FindEntityByName("Crate")
Harness.Expect(crate ~= nil, "the smoke scene has a Crate")

local x0 = Transform.GetPosition(crate)
Harness.Step(60)
local x1 = Transform.GetPosition(crate)
Harness.Log(string.format("crate x: %.3f -> %.3f", x0, x1))
Harness.Expect(x1 - x0 > 0.5, "mover.lua slides the crate along +X")
