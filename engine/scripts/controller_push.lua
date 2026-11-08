-- controller_push.lua — a CharacterController pushes the dynamic Rigidbodies it
-- walks into (#1879): Unity's documented `OnControllerColliderHit` example.
--
-- A CharacterController pushes nothing on its own: its `Move` stops at a crate
-- as at a wall. Attach this script next to the controller and each body it walks
-- into gets the walk's horizontal direction as its velocity, scaled by
-- `push_power`, keeping its own vertical speed (so it still falls). Kinematic
-- bodies, static geometry and hits while moving down (standing on a crate) are
-- left alone. Copy it to make a door that refuses to be pushed, or a push that
-- scales with mass.

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
    -- Don't push what is under us: a move pressing down onto it.
    if hit.moveDirection.y < -0.3 then return end
    local power = Push.push_power
    local _, vy, _ = Physics.GetVelocity(body)
    Physics.SetVelocity(body, hit.moveDirection.x * power, vy, hit.moveDirection.z * power)
end

return Push
