-- scenarios/long.lua — a bot that always dodges runs for a minute of game time at
-- the rising speed, accelerating twice as fast as the default so it reaches the top
-- speed within it (a two-minute run at the default rate cost the game's test about
-- 16 s of CPU for the same proof, #2350). Chunks are built ahead and destroyed
-- behind for the whole run, so the entity count stays under a fixed bound instead
-- of growing with the distance. It takes the shot of a run at speed.

local Bot = require("runner.bot")

local BOUND = 250 -- entities: the scene, ~7 chunks of track and their crates, the effects
local game_id = Scene.FindEntityByName("Game")
Harness.Step(1)
local game = Scene.GetScript(game_id, "runner")
game.accel = 2 * game.accel

local ticks, most, most_at_half = 0, 0, 0
Bot.play(game, 3600, function()
    ticks = ticks + 1
    if ticks % 60 == 0 then
        local n = Debug.Stats().entities.last
        most = math.max(most, n)
        if ticks == 1800 then most_at_half = most end
    end
end)
Harness.Screenshot("cache/test/shots/at_speed.png", { settle = 4 })

Harness.Log(string.format("%d ticks, %.0f m at %.1f m/s, entities at most %d (%d in the first half), %d crates dropped, %d near misses, over %s (%s)",
    ticks, game.distance, game.speed, most, most_at_half, game.dropped, game.near_misses,
    tostring(game.over), tostring(game.killer)))
Harness.Expect(not game.over and ticks == 3600, "the bot ran the whole minute")
Harness.Expect(game.speed >= game.max_speed - 0.01, "the run reached its top speed")
Harness.Expect(most > 0 and most < BOUND, "the entity count stayed under " .. BOUND .. ", at most " .. most)
Harness.Expect(most <= most_at_half + 20, "the second half held no more entities than the first")
