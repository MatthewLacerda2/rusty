-- snowman.lua — a deathmatch bot: it fights every other fighter, bots and the
-- player alike. It knows only what its AIPerception saw, heard or was hit by
-- (the engine's engine/scripts/bot.lua chases the Player alone, so it does not
-- fit an everyone-against-everyone match).
--
--   * Sees someone: turns to face it and fires (combat.lua's gun, with spread
--     and recoil), closing in to a spot an EnvironmentQuery picks around it.
--   * Knows of someone it can't see (heard a shot, was hit, lost sight of it):
--     walks to where it last was.
--   * Knows of no one: roams between the spawn points, the upper floor and the
--     crate top, picked at random from the seeded stream.
--   * Hit and below half health with a grenade left: throws its smoke at its
--     feet. Dies at 0 health and respawns (combat.lua).

local Combat = require("combat")

local Snowman = {}

local REQUERY = 1.0 -- seconds between environment queries while fighting

-- Where to fight from: reachable, in sight of the target (smoke blocks it),
-- near the bot, away from the others' spots. Built in Start: the inspector
-- reads this file's `fields` without running `require`.
local function spot_query()
    return {
        generator = { donut = { context = "target", inner = 4, outer = 10, rings = 2, per_ring = 12 } },
        tests = {
            { path = "exists" },
            { trace = { from = "target", visible = true, item_height = Combat.EYE,
                        mask = 1 | (1 << Combat.SMOKE) } },
            { distance = { to = "querier" }, score = { curve = "inverse", weight = 1 } },
            { separation = { radius = 3 }, score = { weight = 0.5 } },
        },
    }
end

function Snowman.Start(id)
    Combat.start()
    Snowman.health = Combat.HEALTH
    Snowman.gun = Combat.gun({ rate = 0.15, spread = 0.035, kick = 0.012, recover = 0.12 })
    Snowman.smokes = 1
    Snowman.planned_at = -math.huge
    Snowman.spot = spot_query()
    NavMeshAgent.SetUpdateRotation(id, false)
end

local function face(id, dx, dz)
    if dx * dx + dz * dz > 1e-6 then
        Transform.SetRotation(id, 0, math.deg(math.atan(dx, dz)), 0)
    end
end

-- The nearest fighter this bot knows of that is alive, and whether it sees it.
local function enemy(id)
    for _, known in ipairs(AIPerception.GetKnown(id)) do
        local script = Scene.GetScript(known.id, "snowman") or Scene.GetScript(known.id, "shooter")
        if script ~= nil and (script.health or 0) > 0 then
            return known, AIPerception.CanSee(id, known.id)
        end
    end
    return nil, false
end

local function roam(id)
    if NavMeshAgent.HasPath(id) and not NavMeshAgent.IsAtTarget(id) then return end
    local places = Scene.GetChildren(Combat.named("Spawns"))
    for _, w in ipairs(Scene.GetChildren(Combat.named("Waypoints"))) do
        places[#places + 1] = w
    end
    local x, y, z = Transform.GetPosition(places[Random.Range(1, #places + 1)])
    NavMeshAgent.SetTarget(id, x, y, z)
end

local function fight(id, known, now)
    local x, y, z = Transform.GetPosition(id)
    local tx, ty, tz = Transform.GetPosition(known.id)
    local dx, dy, dz = tx - x, (ty + 1.2) - (y + Combat.EYE), tz - z
    face(id, dx, dz)
    local len = math.sqrt(dx * dx + dy * dy + dz * dz)
    Combat.fire(Snowman.gun, id, x, y + Combat.EYE, z, dx / len, dy / len, dz / len)
    if now - Snowman.planned_at >= REQUERY then
        Snowman.planned_at = now
        local found, sx, sy, sz = EnvironmentQuery.Run(id, Snowman.spot, { target = { tx, ty, tz } })
        if found then NavMeshAgent.SetTarget(id, sx, sy, sz) end
    end
end

function Snowman.Update(id, dt)
    local match = Combat.match()
    if match ~= nil and match.over then
        NavMeshAgent.ResetPath(id)
        return
    end
    if Combat.dead(Snowman, id, dt, NavMeshAgent.Warp) then
        NavMeshAgent.ResetPath(id)
        return
    end
    Combat.tick(Snowman.gun, dt)
    local known, sees = enemy(id)
    if sees then
        fight(id, known, Time.time())
    elseif known ~= nil then
        NavMeshAgent.SetTarget(id, known.x, known.y, known.z)
    else
        roam(id)
    end
    if not sees then
        local vx, _, vz = NavMeshAgent.GetVelocity(id)
        face(id, vx, vz)
    end
end

function Snowman.OnTakeDamage(id, amount, event)
    local taken = Combat.hurt(Snowman, id, amount, event)
    if Snowman.health > 0 and Snowman.health < Combat.HEALTH / 2 and Snowman.smokes > 0 then
        Snowman.smokes = 0
        local x, y, z = Transform.GetPosition(id)
        Combat.smoke(x, y, z)
    end
    return taken
end

return Snowman
