-- scenarios/long.lua — a bot that always dodges runs for two minutes of game time
-- at the rising speed. Chunks are built ahead and destroyed behind for the whole
-- run, so the entity count stays under a fixed bound instead of growing with the
-- distance. It takes the shot of a run at speed.

local Bot = require("runner.bot")

local BOUND = 250 -- entities: the scene, ~7 chunks of track and their crates, the effects
local game_id = Scene.FindEntityByName("Game")
Harness.Step(1)
local game = Scene.GetScript(game_id, "runner")

local ticks, most, most_at_minute = 0, 0, 0
Bot.play(game, 7200, function()
    ticks = ticks + 1
    if ticks % 60 == 0 then
        local n = Debug.Stats().entities.last
        most = math.max(most, n)
        if ticks == 3600 then most_at_minute = most end
    end
end)
Harness.Screenshot("cache/test/shots/at_speed.png", { settle = 4 })

Harness.Log(string.format("%d ticks, %.0f m at %.1f m/s, entities at most %d (%d in the first minute), %d crates dropped, %d near misses, over %s (%s)",
    ticks, game.distance, game.speed, most, most_at_minute, game.dropped, game.near_misses,
    tostring(game.over), tostring(game.killer)))
Harness.Expect(not game.over and ticks == 7200, "the bot ran the whole two minutes")
Harness.Expect(game.speed >= game.max_speed - 0.01, "the run reached its top speed")
Harness.Expect(most > 0 and most < BOUND, "the entity count stayed under " .. BOUND .. ", at most " .. most)
Harness.Expect(most <= most_at_minute + 20, "the second minute held no more entities than the first")
