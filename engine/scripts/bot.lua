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

-- Where to stand (#1892): an environment query around the player, reachable,
-- with a line of sight to the player and away from the other bots' spots, so a
-- group spreads round the player instead of funnelling into one file. The
-- `separation` test reads the other agents' NavMeshAgent targets, so each bot's
-- target is its claim. Built once: the player is a context, not part of the spec.
local SPOT = {
    generator = { donut = { context = "target", inner = 3, outer = 8, rings = 2, per_ring = 16 } },
    tests = {
        { path = "exists" },
        { trace = { from = "target", visible = true, item_height = 1.0 } },
        { distance = { to = "querier" }, score = { curve = "inverse", weight = 0.5 } },
        { separation = { radius = 3 }, score = { weight = 1 } },
    },
}
-- Re-query about once a second, or sooner when the player moved this far (m).
local REQUERY_SECONDS = 1.0
local REQUERY_MOVE = 2.0

-- Per bot: when it last queried and where the player stood then.
local plans = {}

-- Pick a spot around the player, or chase the player itself when none is found
-- (no navmesh yet, or the player boxed in).
local function choose_spot(entity_id, player_id, px, py, pz)
    local found, x, y, z = EnvironmentQuery.Run(entity_id, SPOT, { target = player_id })
    if found then
        return x, y, z
    end
    return px, py, pz
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

    local px, py, pz = Transform.GetPosition(player_id)
    local plan = plans[entity_id]
    local moved = plan and ((px - plan.px) ^ 2 + (pz - plan.pz) ^ 2) > REQUERY_MOVE ^ 2
    if plan == nil or moved or Time.time() - plan.at >= REQUERY_SECONDS then
        local x, y, z = choose_spot(entity_id, player_id, px, py, pz)
        NavMeshAgent.SetTarget(entity_id, x, y, z)
        plans[entity_id] = { at = Time.time(), px = px, pz = pz }
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
