-- scenarios/drop.lua — crates and barrels dropped on the track tumble, settle
-- within a few seconds, stay on the track, and the same seed puts them in the
-- same resting places: two runs, each on a freshly loaded scene, are compared
-- crate by crate.

local game
Harness.Step(1)

-- Load the scene, hold the runner at the start and drop three rows ahead of it.
local function drop()
    Scene.Load("assets/scenes/runner.scene")
    Harness.Step(2)
    game = Scene.GetScript(Scene.FindEntityByName("Game"), "runner")
    game.hold = true
    Harness.Step(1)
    local n = game:Drop(50, { -1, 1 }) + game:Drop(62, { 0 }) + game:Drop(70, { -1, 0, 1 })
    Harness.Step(240)
    return n, game:Crates()
end

local n1, first = drop()
local n2, second = drop()
Harness.Log(string.format("dropped %d then %d bodies; %d came to rest on something", n1, n2, game.landings))
Harness.Expect(n1 >= 5 and n1 == n2, "both runs dropped the same bodies, got " .. n1 .. " and " .. n2)

local settled, on_track, same = true, true, true
for i, c in ipairs(first) do
    local d = second[i]
    if c.speed > 0.05 then settled = false end
    if math.abs(c.x) > 4.1 or c.y < 0 or c.y > 3 then on_track = false end
    if not d or math.abs(c.x - d.x) > 1e-3 or math.abs(c.y - d.y) > 1e-3 or math.abs(c.z - d.z) > 1e-3 then
        same = false
    end
    Harness.Log(string.format("#%d at %.3f %.3f %.3f, speed %.4f; second run %.3f %.3f %.3f", c.n, c.x, c.y, c.z, c.speed, d and d.x or 0, d and d.y or 0, d and d.z or 0))
end
Harness.Expect(#first == n1, "every dropped body is still on the track")
Harness.Expect(settled, "every body is at rest four seconds after the drop")
Harness.Expect(on_track, "every body rests on the track, between the rails")
Harness.Expect(same, "the same seed puts every body in the same resting place")
Harness.Expect(game.landings >= n1, "every body's landing raised dust")
