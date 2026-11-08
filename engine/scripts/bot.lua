-- AI-generated bot.lua script expected by the engine
local BotAI = {}

BotAI.health = 100.0

-- The NavMeshAgent's speed, acceleration and stopping distance are authored on
-- Enemy_1's component (the inspector card), as is where it stands: the script reads
-- them, it never overrides them.
function BotAI.Start(entity_id)
    Animator.Play(entity_id, "Walk")
    print("[Lua] Bot initialized")
end

-- Where to stand (#1892): an environment query around where the bot last knew
-- the player to be, reachable, with a line of sight to that spot and away from
-- the other bots' spots, so a group spreads round it instead of funnelling into
-- one file. The `separation` test reads the other agents' NavMeshAgent targets,
-- so each bot's target is its claim. Built once: the spot is a context.
local SPOT = {
    generator = { donut = { context = "target", inner = 3, outer = 8, rings = 2, per_ring = 16 } },
    tests = {
        { path = "exists" },
        { trace = { from = "target", visible = true, item_height = 1.0 } },
        { distance = { to = "querier" }, score = { curve = "inverse", weight = 0.5 } },
        { separation = { radius = 3 }, score = { weight = 1 } },
    },
}
-- Re-query about once a second, or sooner when the last-known position moved
-- this far (m).
local REQUERY_SECONDS = 1.0
local REQUERY_MOVE = 2.0

-- Per bot: when it last queried and the last-known position it queried around.
local plans = {}

-- Pick a spot around the last-known position `(kx, ky, kz)`, or head for that
-- position itself when none is found (no navmesh yet, or the spot boxed in).
local function choose_spot(entity_id, kx, ky, kz)
    local found, x, y, z = EnvironmentQuery.Run(entity_id, SPOT, { target = { kx, ky, kz } })
    if found then
        return x, y, z
    end
    return kx, ky, kz
end

function BotAI.Update(entity_id, delta_time)
    -- A dead bot stops chasing (and querying), so its death animation is not
    -- walked over.
    if BotAI.health <= 0.0 then
        NavMeshAgent.ResetPath(entity_id)
        return
    end
    local player_id = Scene.FindEntityByName("Player")
    if player_id == nil then
        return
    end

    -- Chase what the bot KNOWS (#1900): where its AIPerception last saw, heard
    -- or was shot by the player, never the player's live transform. A bot that
    -- knows nothing keeps the course it has: its authored NavMeshAgent target (a
    -- patrol point; Enemy_1's is the pool's middle), or, once it has forgotten
    -- the player, the last spot it chose, where it searches.
    local known = AIPerception.Get(entity_id, player_id)
    if known ~= nil then
        local plan = plans[entity_id]
        local moved = plan and ((known.x - plan.kx) ^ 2 + (known.z - plan.kz) ^ 2) > REQUERY_MOVE ^ 2
        if plan == nil or moved or Time.time() - plan.at >= REQUERY_SECONDS then
            local x, y, z = choose_spot(entity_id, known.x, known.y, known.z)
            NavMeshAgent.SetTarget(entity_id, x, y, z)
            plans[entity_id] = { at = Time.time(), kx = known.x, kz = known.z }
        end
    end

    -- The agent turns itself to face where it steers, at its authored angular speed
    -- (Update Rotation on the inspector card); the script only picks the animation.
    local vx, vy, vz = NavMeshAgent.GetVelocity(entity_id)
    local speed_sq = vx * vx + vz * vz
    if speed_sq > 0.01 then
        Animator.Play(entity_id, "Walk")
    else
        Animator.Play(entity_id, "Idle")
    end
end

-- OnTakeDamage: the engine delivers every Damage.Apply / ApplyPoint / ApplyRadial
-- that reaches this bot (a hit on any of its colliders, or a child's, bubbles up
-- to it). Health is this script's: the engine stores none. The return value is
-- the damage actually taken.
function BotAI.OnTakeDamage(entity_id, amount, event)
    if BotAI.health <= 0.0 then
        return 0
    end
    BotAI.health = BotAI.health - amount
    print("[Lua] Bot took " .. amount .. " " .. event.type .. " damage! Remaining HP: " .. BotAI.health)

    if BotAI.health <= 0.0 then
        Animator.Play(entity_id, "Death")
        print("[Lua] Bot is DEAD! Triggering death animation.")
    else
        Animator.Play(entity_id, "Hit")
    end
    return amount
end

-- OnTrigger callback hook triggered when this entity overlaps with a trigger
-- collider (`is_trigger`). Solid walls and floors do not fire it (#448).
function BotAI.OnTrigger(self_id, other_id)
    print("[Lua] 🟢 OnTrigger overlap event! Entity " .. self_id .. " intersected trigger with entity " .. other_id)
end

-- OnCollisionEnter fires once when a solid contact begins; `contact` carries the
-- point, normal, relativeVelocity, impulse and otherBody. The Player is
-- kinematic, so only dynamic bodies hitting it raise this (a kinematic body
-- against static walls never collides, as in Unity).
function BotAI.OnCollisionEnter(self_id, other_id, contact)
    print("[Lua] 💥 OnCollisionEnter with entity " .. other_id .. ", impulse " .. contact.impulse)
end

return BotAI
