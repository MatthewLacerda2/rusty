
local Thrower = {
    fields = {
        balls = { type = "number", default = 5, tooltip = "Balls in this level" },
        speed = { type = "number", default = 16, range = { 1, 60 }, tooltip = "Metres per second" },
    },
}

local GRAVITY = 9.81
local RADIUS = 0.15 -- metres, a ball's radius
local MASS = 2.0    -- kilograms

function Thrower.Start(id)
    Thrower.left = Thrower.balls
    Input.SetCursorLocked(false)
    Input.SetCursorVisible(true)
end

-- Aimed straight at the point, plus the lift gravity takes away over the flight.
-- Returns the ball, or nil when none are left.
function Thrower.ThrowAt(x, y, z)
    if Thrower.left <= 0 then return nil end
    Thrower.left = Thrower.left - 1
    local ox, oy, oz = Camera.GetPosition()
    local dx, dy, dz = x - ox, y - oy, z - oz
    local flight = math.sqrt(dx * dx + dy * dy + dz * dz) / Thrower.speed
    local vx, vy, vz = dx / flight, dy / flight + 0.5 * GRAVITY * flight, dz / flight

    local ball = Scene.CreateEntity("Ball", "Sphere")
    Transform.SetPosition(ball, ox, oy - 0.3, oz)
    Transform.SetLocalScale(ball, 2 * RADIUS, 2 * RADIUS, 2 * RADIUS)
    Material.Set(ball, "ball")
    Scene.AddComponent(ball, "Collider")
    Physics.SetColliderShape(ball, { kind = "Sphere", radius = 0.5 })
    Scene.AddComponent(ball, "RigidBody")
    Physics.SetMass(ball, MASS)
    Physics.SetCollisionDetection(ball, "Continuous")
    Physics.SetVelocity(ball, vx, vy, vz)
    Scene.AddScript(ball, "assets/scripts/ball.lua")
    return ball
end

function Thrower.Update(id, dt)
    if Input.GetKeyDown("R") then
        Scene.Load(Scene.GetActivePath())
        return
    end
    if not Input.GetKeyDown("Mouse0") or UI.IsPointerConsumed() then return end
    local ox, oy, oz, dx, dy, dz = Camera.MouseRay()
    local hit, _, dist = Physics.Raycast(ox, oy, oz, dx, dy, dz, nil, nil, 60)
    if not hit then dist = 30 end
    Thrower.ThrowAt(ox + dx * dist, oy + dy * dist, oz + dz * dist)
end

return Thrower
