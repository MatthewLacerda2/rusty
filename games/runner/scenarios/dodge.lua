-- scenarios/dodge.lua — the bot dodges the fixed seed's first obstacles through
-- the player's own keys and survives 500 m, jumping, sliding and changing lanes
-- on the way. It takes the shot of a lane change in mid-air.

local Bot = require("runner.bot")
local Track = require("runner.track")

local game_id = Scene.FindEntityByName("Game")
Harness.Step(1)
local game = Scene.GetScript(game_id, "runner")

local shot = false
Bot.play(game, 3600, function(view)
    local off = math.abs(view.x - Track.lane_x(view.lane))
    if not shot and not view.grounded and off > 0.5 and view.feet > 0.4 then
        shot = true
        Harness.Screenshot("cache/test/shots/lane_change_mid_air.png", { settle = 4 })
    end
    if view.z >= 500 then game.hold = true end
end)

Harness.Log(string.format("ran %.1f m at %.1f m/s: %d jumps, %d slides, %d lane changes, %d near misses, %d crates dropped, over %s (%s)",
    game.distance, game.speed, game.jumps, game.slides, game.lane_changes, game.near_misses, game.dropped,
    tostring(game.over), tostring(game.killer)))
Harness.Expect(not game.over, "the bot survives")
Harness.Expect(game.distance >= 500, "it ran 500 m")
Harness.Expect(game.jumps > 0 and game.slides > 0 and game.lane_changes > 0,
    "it jumped, slid and changed lanes to get there")
Harness.Expect(game.dropped > 0 and game.landings > 0, "crates were dropped ahead and landed")
Harness.Expect(game.speed > game.start_speed, "the speed rose")
