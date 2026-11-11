-- mover.lua — slides its entity along +X at `speed` metres per second, so a
-- scenario can prove that a game script runs and moves something.

local Mover = {
    fields = {
        speed = { type = "number", default = 1.0, range = { 0, 10 },
            tooltip = "Metres per second along +X" },
    },
}

function Mover.Update(id, dt)
    local x, y, z = Transform.GetPosition(id)
    Transform.SetPosition(id, x + Mover.speed * dt, y, z)
end

return Mover
