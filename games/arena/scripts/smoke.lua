-- smoke.lua — a smoke grenade's cloud (combat.lua throws it): it blocks the
-- bots' sight for `seconds`, stops emitting, and is gone once its last puffs fade.

local Smoke = {
    fields = {
        seconds = { type = "number", default = 12, range = { 1, 60 },
            tooltip = "How long the cloud blocks sight" },
    },
}

function Smoke.Start(id)
    Smoke.left = Smoke.seconds
end

function Smoke.Update(id, dt)
    Smoke.left = Smoke.left - dt
    if Smoke.left <= 0 and Particles.IsActive(id) then
        Particles.SetActive(id, false)
        Scene.RemoveComponent(id, "Collider")
    elseif Smoke.left <= -3 then
        Scene.DestroyEntity(id)
    end
end

return Smoke
