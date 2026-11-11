-- roll.lua — rolls its ball along +X at `speed` metres per second until `to_x`,
-- turning it about Z as a ball of `radius` turns when it rolls without slipping.

local Roll = {
    fields = {
        speed = { type = "number", default = 1.5, range = { 0, 10 },
            tooltip = "Metres per second along +X" },
        radius = { type = "number", default = 0.3, range = { 0.01, 10 },
            tooltip = "The ball's radius, which sets how fast it turns" },
        to_x = { type = "number", default = 5.0, range = { -100, 100 },
            tooltip = "Where along X the ball stops" },
    },
}

function Roll.Update(id, dt)
    local x, y, z = Transform.GetPosition(id)
    if x >= Roll.to_x then return end
    local nx = math.min(x + Roll.speed * dt, Roll.to_x)
    Transform.SetPosition(id, nx, y, z)
    local _, _, angle = Transform.GetRotation(id)
    Transform.SetRotation(id, 0, 0, angle - math.deg((nx - x) / Roll.radius))
end

return Roll
