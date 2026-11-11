-- figure.lua — on every part of a jointed figure (games/topple/recipe.lua).
-- The joints' drives hold the figure standing; a hard hit on any part, or a
-- blast, turns every drive off and the figure falls as a ragdoll.

local Figure = {}

local HARD = 3.0 -- N·s, the impulse of a hit that knocks the figure limp

local function limp(id)
    local group = Scene.GetParent(id)
    for _, part in ipairs(Scene.GetChildren(group)) do
        Joint.SetDriveStrength(part, 0)
        Joint.SetDriveDamping(part, 0)
    end
end

function Figure.OnCollisionEnter(id, other, contact)
    if contact.impulse < HARD or Scene.GetParent(other) == Scene.GetParent(id) then return end
    limp(id)
    local p = contact.point
    Audio.PlayAt("assets/sounds/thud.wav", p.x, p.y, p.z, math.min(1, contact.impulse / 20))
end

function Figure.OnTakeDamage(id, amount, event)
    limp(id)
    return amount
end

return Figure
