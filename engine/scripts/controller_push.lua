-- Unity's documented `OnControllerColliderHit` example: a CharacterController's
-- `Move` stops at a crate as at a wall, so this pushes the dynamic body with
-- the walk's horizontal direction, keeping its own vertical speed.

local Push = {
    fields = {
        push_power = { type = "number", default = 2.0, range = { 0, 20 },
            tooltip = "Speed (m/s) a pushed body takes along the walk" },
    },
}

function Push.OnControllerColliderHit(id, hit)
    local body = hit.rigidbody
    if not body then return end -- static geometry, another character
    local rb = Component.Get(body, "RigidBody")
    if not rb or rb.is_kinematic then return end
    if hit.moveDirection.y < -0.3 then return end
    local power = Push.push_power
    local _, vy, _ = Physics.GetVelocity(body)
    Physics.SetVelocity(body, hit.moveDirection.x * power, vy, hit.moveDirection.z * power)
end

return Push
