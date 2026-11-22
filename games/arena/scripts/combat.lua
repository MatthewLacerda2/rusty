-- Required by bots (snowman.lua) and the player (shooter.lua); defines no callback.

local Combat = {}

Combat.FIGHTER = 8   -- the recipe sets these layers too: change both
Combat.SMOKE = 9     -- blocks sight, not bullets
Combat.EYE = 1.6     -- m, feet to eye
Combat.HEALTH = 100

local SHOT_MASK = 1 | (1 << Combat.FIGHTER)
local RANGE = 60       -- m
local DAMAGE = 20
local RESIST = 4       -- shot power lost per metre of solid
local RESPAWN = 1.0    -- s
local MAX_KICKS = 4
local RECOIL_CLIMB = 0.5
local MUZZLE_AHEAD = 0.6  -- m
local MUZZLE_BELOW_EYE = 0.4

-- A gun's state: `rate` seconds between shots, `spread` radians of aim error,
-- `kick` radians of recoil each shot adds, recovering at `recover` per second.
function Combat.gun(opts)
    return { cooldown = 0, recoil = 0, rate = opts.rate, spread = opts.spread,
             kick = opts.kick, max_recoil = MAX_KICKS * opts.kick, recover = opts.recover }
end

function Combat.tick(gun, dt)
    gun.cooldown = math.max(0, gun.cooldown - dt)
    gun.recoil = math.max(0, gun.recoil - gun.recover * dt)
end

local found = {}

-- Every fighter's Start calls this, so a reloaded level is looked up afresh.
function Combat.start()
    found = {}
end

function Combat.named(name)
    if found[name] == nil then found[name] = Scene.FindEntityByName(name) end
    return found[name]
end

local function burst(name, x, y, z, count)
    local id = Combat.named(name)
    if id ~= nil then
        Transform.SetPosition(id, x, y, z)
        Particles.Emit(id, count)
    end
end

-- Fire `gun` from `shooter`'s eye along the unit vector `dx, dy, dz`; `false` while cooling down.
function Combat.fire(gun, shooter, ox, oy, oz, dx, dy, dz)
    if gun.cooldown > 0 then return false end
    gun.cooldown = gun.rate
    local stray = gun.spread + gun.recoil
    local rx, ry, rz = Random.InsideUnitSphere()
    dx, dy, dz = dx + rx * stray, dy + ry * stray + gun.recoil * RECOIL_CLIMB, dz + rz * stray
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
            if power <= 0.05 or not crossing.exit then break end -- under 5 % left
        end
    end
    burst("Flash", ox + dx * MUZZLE_AHEAD, oy - MUZZLE_BELOW_EYE, oz + dz * MUZZLE_AHEAD, 4)
    Audio.PlayAt("assets/sounds/shot.wav", ox, oy, oz, 0.5)
    AIPerception.ReportNoise(ox, oy, oz, 1.0, { instigator = shooter, tag = "gunshot" })
    return true
end

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

function Combat.match()
    return Scene.GetScript(Combat.named("Match"), "match")
end

local function spawn_point(self_id)
    local best, best_d = nil, -1
    local others = Combat.match() and Combat.match().fighters or {}
    for _, spawn in ipairs(Scene.GetChildren(Combat.named("Spawns"))) do
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

-- Call from `Update`: `true` while dead.
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
