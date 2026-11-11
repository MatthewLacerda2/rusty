-- ball.lua — a thrown ball: a knock, a puff of dust and a scuff where it
-- strikes something hard.

local Ball = {}

local HARD = 4.0 -- N·s, the impulse of a strike worth marking

function Ball.OnCollisionEnter(id, other, contact)
    if contact.impulse < HARD then return end
    local p, n = contact.point, contact.normal
    Audio.PlayAt("assets/sounds/knock.wav", p.x, p.y, p.z, math.min(1, contact.impulse / 30))
    -- The normal points out of `other`: the struck surface's own normal.
    Decals.Spawn(p.x, p.y, p.z, n.x, n.y, n.z,
        { size = 0.35, texture = "assets/textures/puff.png", color = { 0.1, 0.08, 0.06, 0.8 },
          owner = other, lifetime = 20, fade = 2 })
    local dust = Scene.FindEntityByName("Dust")
    if dust then
        Transform.SetPosition(dust, p.x, p.y, p.z)
        Particles.Emit(dust, 12)
    end
end

return Ball
