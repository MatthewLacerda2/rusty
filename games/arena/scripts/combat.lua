-- combat.lua — what every arena fighter shares, the bots (snowman.lua) and the
-- player (shooter.lua): a raycast gun with spread, recoil and wallbangs, the
-- smoke grenade, and health with death and respawn. A plain module, required
-- by both: it defines no callback, so it is not a behaviour of its own.

local Combat = {}

Combat.FIGHTER = 8   -- the layer fighters stand on (the recipe names it)
Combat.SMOKE = 9     -- the layer a smoke cloud is on: blocks sight, not bullets
Combat.EYE = 1.6     -- metres from a fighter's feet to its eye
Combat.HEALTH = 100

local SHOT_MASK = 1 | (1 << Combat.FIGHTER) -- the level and the fighters
local RANGE = 60       -- metres a shot reaches
local DAMAGE = 20      -- what a clean hit deals: five to kill
local RESIST = 4       -- shot power lost per metre of solid crossed: a 10 cm
                       -- wall keeps 60 %, a 40 cm one stops it
local RESPAWN = 1.0    -- seconds a dead fighter waits before it respawns

-- A gun's state: `rate` seconds between shots, `spread` radians of aim error,
-- `kick` radians of recoil each shot adds, recovering at `recover` per second.
function Combat.gun(opts)
    return { cooldown = 0, recoil = 0, rate = opts.rate, spread = opts.spread,
             kick = opts.kick, max_recoil = 4 * opts.kick, recover = opts.recover }
end

function Combat.tick(gun, dt)
    gun.cooldown = math.max(0, gun.cooldown - dt)
    gun.recoil = math.max(0, gun.recoil - gun.recover * dt)
end

local function burst(name, x, y, z, count)
    local id = Scene.FindEntityByName(name)
    if id ~= nil then
        Transform.SetPosition(id, x, y, z)
        Particles.Emit(id, count)
    end
end

-- Fire `gun` from `shooter`'s eye at `ox, oy, oz` along `dx, dy, dz` (unit).
-- The shot strays by the spread and the recoil built up so far, goes through
-- whatever is thin enough (power falls with each metre of solid), and damages
-- each fighter it reaches. `false` while the gun is cooling down.
function Combat.fire(gun, shooter, ox, oy, oz, dx, dy, dz)
    if gun.cooldown > 0 then return false end
    gun.cooldown = gun.rate
    local stray = gun.spread + gun.recoil
    local rx, ry, rz = Random.InsideUnitSphere()
    dx, dy, dz = dx + rx * stray, dy + ry * stray + gun.recoil * 0.5, dz + rz * stray
    local len = math.sqrt(dx * dx + dy * dy + dz * dz)
    dx, dy, dz = dx / len, dy / len, dz / len
    gun.recoil = math.min(gun.max_recoil, gun.recoil + gun.kick)

    local power = 1.0
    for _, crossing in ipairs(Physics.RaycastThrough(ox, oy, oz, dx, dy, dz, RANGE, SHOT_MASK)) do
        if crossing.root ~= shooter then
            local e = crossing.enter
            burst("Sparks", e.point.x, e.point.y, e.point.z, 6)
            Damage.ApplyPoint(crossing, DAMAGE * power, dx, dy, dz,
                { type = "bullet", instigator = shooter })
            power = power - crossing.thickness * RESIST
            if power <= 0.05 or not crossing.exit then break end
        end
    end
    burst("Flash", ox + dx * 0.6, oy - 0.4, oz + dz * 0.6, 4)
    Audio.PlayAt("assets/sounds/shot.wav", ox, oy, oz, 0.5)
    AIPerception.ReportNoise(ox, oy, oz, 1.0, { instigator = shooter, tag = "gunshot" })
    return true
end

-- Throw a smoke grenade: a cloud lands at `x, y, z` (scripts/smoke.lua).
function Combat.smoke(x, y, z)
    local cloud = Scene.CreateEntity("Smoke")
    Scene.AddComponent(cloud, "Collider")
    Physics.SetColliderShape(cloud, { kind = "Sphere", radius = 0.5 })
    Transform.SetPosition(cloud, x, y + 1.5, z)
    Transform.SetLocalScale(cloud, 5, 4, 5)
    Layers.SetLayer(cloud, Combat.SMOKE)
    Scene.AddComponent(cloud, "Particles")
    Particles.SetShape(cloud, "Sphere", { radius = 0.45 })
    Particles.SetRate(cloud, 40)
    Particles.SetLifetime(cloud, 2, 3)
    Particles.SetSpeed(cloud, 0.1, 0.4)
    Particles.SetSize(cloud, 1.2, 2.0)
    Particles.SetColor(cloud, 0.75, 0.75, 0.75, 0.6)
    Component.Set(cloud, "Particles", { texture = "assets/textures/puff.png" })
    Scene.AddScript(cloud, "assets/scripts/smoke.lua")
    return cloud
end

-- The match's score keeper (scripts/match.lua), or `nil` before it loads.
function Combat.match()
    return Scene.GetScript(Scene.FindEntityByName("Match"), "match")
end

-- The spawn point farthest from every other fighter: where a dead one comes back.
local function spawn_point(self_id)
    local best, best_d = nil, -1
    local others = Combat.match() and Combat.match().fighters or {}
    for _, spawn in ipairs(Scene.GetChildren(Scene.FindEntityByName("Spawns"))) do
        local sx, sy, sz = Transform.GetPosition(spawn)
        local d = math.huge
        for _, other in ipairs(others) do
            if other ~= self_id then
                local ox, oy, oz = Transform.GetPosition(other)
                d = math.min(d, (ox - sx) ^ 2 + (oy - sy) ^ 2 + (oz - sz) ^ 2)
            end
        end
        if d > best_d then best, best_d = spawn, d end
    end
    return Transform.GetPosition(best)
end

-- `OnTakeDamage` for a fighter whose health is `self.health`: takes the hit,
-- and on the killing one scores it and starts the respawn wait.
function Combat.hurt(self, id, amount, event)
    if self.health <= 0 then return 0 end
    self.health = self.health - amount
    if self.health <= 0 then
        self.respawn_in = RESPAWN
        local match = Combat.match()
        if match then match.Killed(id, event.instigator) end
    end
    return amount
end

-- Call from `Update`: while dead, count down, then come back at full health at
-- a spawn point (`warp` moves the body there). `true` while dead.
function Combat.dead(self, id, dt, warp)
    if self.health > 0 then return false end
    self.respawn_in = self.respawn_in - dt
    if self.respawn_in <= 0 then
        warp(id, spawn_point(id))
        self.health = Combat.HEALTH
    end
    return true
end

return Combat
