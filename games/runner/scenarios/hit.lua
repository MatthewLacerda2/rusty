-- scenarios/hit.lua — no inputs: the runner keeps to the middle lane, and the
-- first hazard in that lane ends the run there, not later and not never.

local game_id = Scene.FindEntityByName("Game")
Harness.Step(1)
local game = Scene.GetScript(game_id, "runner")
local Track = require("runner.track")

-- The front of every hazard in the middle lane, nearest first, while the run lasts.
local first = math.huge
local ticks = 0
while not game.over and ticks < 1800 do
    for _, h in ipairs(game:View().hazards) do
        if h.min[1] < Track.lane_x(0) + 0.4 and h.max[1] > Track.lane_x(0) - 0.4 then
            first = math.min(first, h.min[3])
        end
    end
    Harness.Step(1)
    ticks = ticks + 1
end
Harness.Log(string.format("crashed into a %s after %d ticks at %.2f m; the first hazard in the lane at %.2f m",
    tostring(game.killer), ticks, game.distance, first))
Harness.Expect(game.over, "the run ends on a hazard")
Harness.Expect(game.distance > Track.START - 1, "the start pad is clear: it ran past it")
Harness.Expect(game.distance < first and game.distance > first - 1.5,
    "it stopped at the first hazard in its lane, not through it")
Harness.Expect(game.jumps == 0 and game.slides == 0 and game.lane_changes == 0, "nothing was pressed")

-- The run is over: the runner stays put, the banner shows, and Enter runs again.
local at = game.distance
Harness.Step(30)
Harness.Expect(game.distance == at, "a crashed runner does not move on")
local banner = Scene.FindChild(Scene.FindEntityByName("HUD"), "GameOver")
Harness.Expect(Scene.IsActiveInHierarchy(banner), "the crash banner is shown")
Input.Press("ENTER")
Harness.Step(1)
Input.Release("ENTER")
Harness.Step(2)
Harness.Expect(not game.over and game.distance < 1, "Enter starts a new run from the start")
