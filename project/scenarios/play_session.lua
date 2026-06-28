-- project/scenarios/play_session.lua — drive a full bot-played session, headless.
--
-- Run it with:
--   cargo run --bin play --features dev -- project/scenarios/play_session.lua out/
--
-- This is the issue-#10 end-to-end agentic loop: attach the DEV-ONLY bot-player to the
-- Player, let it play the demo match on its own (navigate -> aim -> close on the enemy),
-- and emit a deterministic summary (won/lost, time, errors). The bot drives writable
-- Input from its Update(); the scenario here is just the referee. The win condition is
-- a proximity demo: the bot has closed to within ARRIVE_RANGE of the enemy.

local MAX_FRAMES = 1800          -- 30 s @ 1/60 — hard cap so a stuck bot still reports
local ARRIVE_RANGE = 4.0         -- "close enough" — matches the bot's settle distance
local PLAYER = "Player"
local ENEMY = "Enemy_1"

local player = Scene.FindEntityByName(PLAYER)
local enemy = Scene.FindEntityByName(ENEMY)
Harness.Expect(player ~= nil, "demo scene has a Player entity")
Harness.Expect(enemy ~= nil, "demo scene has an Enemy_1 entity")

-- Wire the bot onto the Player BEFORE the first Step: scripts load when the world
-- enters play mode on the first tick.
local attached = Harness.AttachPlayerBot("project/scripts/bot_player.lua")
Harness.Expect(attached, "bot_player.lua attached to the Player")

-- Ground-plane distance between the Player and the enemy.
local function dist_to_enemy()
    local px, _, pz = Transform.GetPosition(player)
    local ex, _, ez = Transform.GetPosition(enemy)
    local dx, dz = ex - px, ez - pz
    return math.sqrt(dx * dx + dz * dz)
end

Harness.Log(string.format("start distance: %.1f", dist_to_enemy()))

-- Let the bot play until it has closed on the enemy or we hit the cap. One StepUntil
-- skips the whole match and we observe exactly once at the end — the harness's core loop.
local won = Harness.StepUntil(function()
    return dist_to_enemy() <= ARRIVE_RANGE
end, MAX_FRAMES)

local frames = Harness.Frame()
local end_dist = dist_to_enemy()

Harness.Log(string.format("result: %s", won and "WON" or "LOST"))
Harness.Log(string.format("time: %d frames (%.2fs @ 1/60)", frames, frames / 60.0))
Harness.Log(string.format("end distance: %.1f (arrive <= %.1f)", end_dist, ARRIVE_RANGE))

-- Acceptance: the bot played autonomously and closed on the enemy within the cap.
Harness.Expect(won, "bot closed on the enemy within the time cap")
Harness.Expect(end_dist <= ARRIVE_RANGE, "bot is within ARRIVE_RANGE at end of session")
Harness.Expect(frames < MAX_FRAMES, "session ended before the hard frame cap")

Harness.Log("play_session complete")
