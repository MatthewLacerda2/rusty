-- Feeds the HUD (hud.lua) a firefight's worth of changes: a shot a tick from a
-- magazine of MAG, a damage number on a soldier every HIT_EVERY ticks, a hurt every
-- HURT_EVERY, by HURT: not whole, as a wallbang's damage is. Deterministic, as the
-- bench is.

local MAG = 30
local HIT_EVERY = 6
local HURT_EVERY = 45
local HURT = 14.75
local SOLDIERS = 50

local Feed = {}

local tick = 0
local hud

function Feed.Start(id)
    hud = Scene.GetScript(Scene.FindEntityByName("HUD"), "hud")
    assert(hud, "hud_feed: no HUD entity running hud.lua")
end

function Feed.Update(id, dt)
    tick = tick + 1
    local shot = tick % MAG
    hud.Fired()
    hud.SetAmmo(MAG - shot, 90 - (tick // MAG) % 4 * MAG)
    hud.SetSpread(0.004 + 0.002 * (shot % 4))
    if tick % HIT_EVERY == 0 then
        local soldier = Scene.FindEntityByName("Soldier_" .. (tick // HIT_EVERY) % SOLDIERS)
        if soldier then
            local x, y, z = Transform.GetPosition(soldier)
            hud.Hit(x, y + 1.6, z, 20)
        end
    end
    if tick % HURT_EVERY == 0 then
        hud.Hurt(15)
        hud.SetHealth(100 - (tick // HURT_EVERY) % 6 * HURT, 100)
    end
end

return Feed
