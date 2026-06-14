-- project/scenarios/play_session.lua — drive a full bot-played session, headless.
--
-- Run it with:
--   cargo run --bin play --features dev -- project/scenarios/play_session.lua out/
--
-- This is the issue-#10 end-to-end agentic loop: attach the DEV-ONLY bot-player to the
-- Player, let it play the demo match on its own (navigate -> aim -> shoot the enemy),
-- and emit a deterministic summary (won/lost, time, remaining health, errors). The bot
-- drives writable Input from its Update(); the scenario here is just the referee.

local MAX_FRAMES = 1800          -- 30 s @ 1/60 — hard cap so a stuck bot still reports
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

local start_hp = Health.Get(enemy)
Harness.Log(string.format("enemy start HP: %.1f", start_hp))

-- Let the bot play until the enemy is dead or we hit the cap. One StepUntil skips the
-- whole match and we observe exactly once at the end — the harness's core loop.
local won = Harness.StepUntil(function()
    return Health.Get(enemy) <= 0.0
end, MAX_FRAMES)

local frames = Harness.Frame()
local enemy_hp = Health.Get(enemy)
local player_hp = Health.Get(player) -- Player has no Health component -> 0 (informational)

Harness.Log(string.format("result: %s", won and "WON" or "LOST"))
Harness.Log(string.format("time: %d frames (%.2fs @ 1/60)", frames, frames / 60.0))
Harness.Log(string.format("enemy remaining HP: %.1f / %.1f", enemy_hp, start_hp))
Harness.Log(string.format("player remaining HP: %.1f", player_hp))

-- Acceptance: the bot played autonomously and finished the match within the cap.
Harness.Expect(won, "bot defeated the enemy within the time cap")
Harness.Expect(enemy_hp <= 0.0, "enemy is dead at end of session")
Harness.Expect(frames < MAX_FRAMES, "session ended before the hard frame cap")

Harness.Log("play_session complete")
