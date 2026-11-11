-- barrel.lua — an explosive barrel: a hard hit or a blast sets it off.
-- The blast is Damage.ApplyRadial's: damage and an impulse away from the
-- barrel on every body within OUTER metres, falling off linearly from INNER.

local Barrel = {}

local HARD = 8.0    -- N·s, the impulse of a hit that sets it off
local OUTER = 3.5   -- metres, the blast's reach
local INNER = 0.5   -- metres, full strength inside it
local IMPULSE = 5   -- N·s, the push at full strength

local function explode(id)
    if Barrel.exploded then return end
    Barrel.exploded = true
    local x, y, z = Transform.GetPosition(id)
    Damage.ApplyRadial(x, y, z, 100, {
        outer = OUTER, inner = INNER, impulse = IMPULSE, type = "explosion", causer = id,
        blocking_mask = 0, -- the bodies don't shield each other
    })
    Audio.PlayAt("assets/sounds/boom.wav", x, y, z, 1.0, 2, 60)
    local fire = Scene.FindEntityByName("Fire")
    if fire then
        Transform.SetPosition(fire, x, y, z)
        Particles.Emit(fire, 60)
    end
    -- A scorch on whatever the barrel stood on.
    local hit, _, _, px, py, pz, nx, ny, nz = Physics.Raycast(x, y, z, 0, -1, 0, id, nil, 3)
    if hit then Decals.Spawn(px, py, pz, nx, ny, nz, { material = "scorch", size = 2.5 }) end
    Scene.DestroyEntity(id)
end

function Barrel.OnCollisionEnter(id, other, contact)
    if contact.impulse >= HARD then explode(id) end
end

-- Another barrel's blast sets this one off too.
function Barrel.OnTakeDamage(id, amount, event)
    explode(id)
    return amount
end

return Barrel
