-- Drive a full bot-played session headless: attach the DEV-ONLY bot-player to the Player and let it
-- play the demo match; the scenario is the referee, emitting a deterministic summary (won/lost,
-- time, errors). Win condition: the bot is within ARRIVE_RANGE of the enemy.

local MAX_FRAMES = 1800          -- 30 s @ 1/60 hard cap
local ARRIVE_RANGE = 4.0          -- metres
local PLAYER = "Player"
local ENEMY = "Enemy_1"

local player = Scene.FindEntityByName(PLAYER)
local enemy = Scene.FindEntityByName(ENEMY)
Harness.Expect(player ~= nil, "demo scene has a Player entity")
Harness.Expect(enemy ~= nil, "demo scene has an Enemy_1 entity")

-- Wire the bot before the first Step: scripts load when play mode starts on the first tick.
local attached = Harness.AttachPlayerBot("assets/scripts/bot_player.lua")
Harness.Expect(attached, "bot_player.lua attached to the Player")

local function dist_to_enemy()
    local px, _, pz = Transform.GetPosition(player)
    local ex, _, ez = Transform.GetPosition(enemy)
    local dx, dz = ex - px, ez - pz
    return math.sqrt(dx * dx + dz * dz)
end

Harness.Log(string.format("start distance: %.1f", dist_to_enemy()))

-- One StepUntil skips the whole match; observe once at the end.
local won = Harness.StepUntil(function()
    return dist_to_enemy() <= ARRIVE_RANGE
end, MAX_FRAMES)

local frames = Harness.Frame()
local end_dist = dist_to_enemy()

Harness.Log(string.format("result: %s", won and "WON" or "LOST"))
Harness.Log(string.format("time: %d frames (%.2fs @ 1/60)", frames, frames / 60.0))
Harness.Log(string.format("end distance: %.1f (arrive <= %.1f)", end_dist, ARRIVE_RANGE))

Harness.Expect(won, "bot closed on the enemy within the time cap")
Harness.Expect(end_dist <= ARRIVE_RANGE, "bot is within ARRIVE_RANGE at end of session")
Harness.Expect(frames < MAX_FRAMES, "session ended before the hard frame cap")

Harness.Log("play_session complete")
