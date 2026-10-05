-- project/scenarios/bench/flicker.lua — a light that flickers like a muzzle flash (#835).
--
-- Bursts of a few frames at four times its intensity, then dark, at seeded
-- intervals: automatic fire, as far as the lighting is concerned.

local Flicker = {}
local state = {} -- per light, by entity id

function Flicker.Start(id)
    state[id] = { base = Light.GetIntensity(id), wait = math.random(1, 20) }
end

function Flicker.Update(id, dt)
    local s = state[id]
    if s == nil then
        return
    end
    s.wait = s.wait - 1
    if s.wait <= 0 then
        local flash = math.random() < 0.5
        Light.SetIntensity(id, flash and s.base * 4 or 0)
        s.wait = math.random(2, 6)
    end
end

return Flicker
