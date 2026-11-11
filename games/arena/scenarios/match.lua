-- scenarios/match.lua — bots only: the deathmatch reaches its kill limit
-- before its time limit, and every bot both scores and dies at least once (no
-- bot stuck somewhere on the map). Takes the mid-fight shot on the way.

Scene.SetActive(Scene.FindEntityByName("Player"), false)
Harness.Step(1)
local match = Scene.GetScript(Scene.FindEntityByName("Match"), "match")
local limit = match.time_limit

local shot = false
local ticks = 0
while not match.over and ticks < (limit + 5) * 60 do
    Harness.Step(30)
    ticks = ticks + 30
    local smoke = Scene.FindEntityByName("Smoke")
    if smoke ~= nil and not shot then
        -- A bot mid-fight with the smoke behind it: the camera beyond the bot
        -- nearest the cloud, looking past it at the cloud.
        shot = true
        local sx, sy, sz = Transform.GetPosition(smoke)
        local near, best = nil, math.huge
        for _, f in ipairs(match.fighters) do
            local x, _, z = Transform.GetPosition(f)
            local d = (x - sx) ^ 2 + (z - sz) ^ 2
            if d > 9 and d < best then near, best = f, d end
        end
        if near ~= nil then
            local bx, by, bz = Transform.GetPosition(near)
            local dx, dz = bx - sx, bz - sz
            local len = math.sqrt(dx * dx + dz * dz)
            Camera.SetPosition(bx + dx / len * 6, by + 2.2, bz + dz / len * 6)
            Camera.SetYaw(math.deg(math.atan(-dz, -dx)))
            Camera.SetPitch(-8)
            Camera.ResetHistory()
            if not Harness.Screenshot("cache/test/shots/fight.png", { settle = 4 }) then
                Harness.Log("shot skipped (no GPU adapter): fight")
            end
        end
    end
end

local kills, deaths = match.kills, match.deaths
for i, f in ipairs(match.fighters) do
    Harness.Log(string.format("%s: %d kills, %d deaths", Scene.GetName(f), kills[f], deaths[f]))
end
Harness.Log(string.format("over after %.1f s of %d, winner %s", match.elapsed, limit,
    tostring(match.winner and Scene.GetName(match.winner))))
Harness.Expect(match.winner ~= nil, "a bot reaches the kill limit")
Harness.Expect(match.elapsed < limit, "within the time limit")
for _, f in ipairs(match.fighters) do
    Harness.Expect(kills[f] >= 1, Scene.GetName(f) .. " scores")
    Harness.Expect(deaths[f] >= 1, Scene.GetName(f) .. " dies")
end
