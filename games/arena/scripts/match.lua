-- match.lua — the deathmatch's score: everyone against everyone. Each death is
-- reported here (combat.lua); the match ends when a fighter reaches the kill
-- limit or the time runs out, and then every fighter stops.

local Match = {
    fields = {
        kill_limit = { type = "number", default = 10, range = { 1, 100 },
            tooltip = "Kills that win the match" },
        time_limit = { type = "number", default = 120, range = { 10, 3600 },
            tooltip = "Seconds the match lasts at most" },
    },
}

function Match.Start(id)
    Match.fighters = {}
    Match.kills, Match.deaths = {}, {}
    for i = 1, math.huge do
        local bot = Scene.FindEntityByName("Bot_" .. i)
        if bot == nil then break end
        Match.fighters[#Match.fighters + 1] = bot
    end
    Match.fighters[#Match.fighters + 1] = Scene.FindEntityByName("Player")
    for _, f in ipairs(Match.fighters) do
        Match.kills[f], Match.deaths[f] = 0, 0
    end
    Match.elapsed = 0
    Match.over = false
    Match.winner = nil
end

function Match.Update(id, dt)
    if Match.over then return end
    Match.elapsed = Match.elapsed + dt
    if Match.elapsed >= Match.time_limit then
        Match.over = true
        print(string.format("[Arena] time is up at %.1f s", Match.elapsed))
    end
end

-- `victim` died to `killer` (`nil`, or itself, for no one's kill).
function Match.Killed(victim, killer)
    if Match.over then return end
    Match.deaths[victim] = (Match.deaths[victim] or 0) + 1
    if killer == nil or killer == victim or Match.kills[killer] == nil then return end
    Match.kills[killer] = Match.kills[killer] + 1
    print(string.format("[Arena] %s killed %s (%d)", Scene.GetName(killer),
        Scene.GetName(victim), Match.kills[killer]))
    if Match.kills[killer] >= Match.kill_limit then
        Match.over = true
        Match.winner = killer
        print(string.format("[Arena] %s wins at %.1f s", Scene.GetName(killer), Match.elapsed))
    end
end

return Match
