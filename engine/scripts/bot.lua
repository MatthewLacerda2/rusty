local BotAI = {}

BotAI.health = 100.0

function BotAI.Start(entity_id)
    Animator.Play(entity_id, "Walk")
    print("[Lua] Bot initialized")
end

-- `separation` reads other agents' NavMeshAgent targets, so a target is a claim.
-- The last-known position is the player's feet: a trace from 1.5 m over it
-- starts inside the player's body, so it traces only against the bot's
-- AIPerception blocking mask (the level), set per bot before each run.
local TRACE = { from = "target", visible = true, item_height = 1.0 }
local SPOT = {
    generator = { donut = { context = "target", inner = 3, outer = 8, rings = 2, per_ring = 16 } },
    tests = {
        { path = "exists" },
        { trace = TRACE },
        { distance = { to = "querier" }, score = { curve = "inverse", weight = 0.5 } },
        { separation = { radius = 3 }, score = { weight = 1 } },
    },
}
-- Re-query at most about once a second unless the last-known position moved this far (m).
local REQUERY_SECONDS = 1.0
local REQUERY_MOVE = 2.0

local plans = {}

local function choose_spot(entity_id, kx, ky, kz)
    TRACE.mask = AIPerception.GetBlockingMask(entity_id)
    local found, x, y, z = EnvironmentQuery.Run(entity_id, SPOT, { target = { kx, ky, kz } })
    if found then
        return x, y, z
    end
    return kx, ky, kz
end

function BotAI.Update(entity_id, delta_time)
    if BotAI.health <= 0.0 then
        NavMeshAgent.ResetPath(entity_id)
        return
    end
    local player_id = Scene.FindEntityByName("Player")
    if player_id == nil then
        return
    end

    -- Chase what the bot KNOWS (perception), never the player's live transform.
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

    local vx, vy, vz = NavMeshAgent.GetVelocity(entity_id)
    local speed_sq = vx * vx + vz * vz
    if speed_sq > 0.01 then
        Animator.Play(entity_id, "Walk")
    else
        Animator.Play(entity_id, "Idle")
    end
end

-- Health is this script's (the engine stores none); returns the damage taken.
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

function BotAI.OnTrigger(self_id, other_id)
    print("[Lua] 🟢 OnTrigger overlap event! Entity " .. self_id .. " intersected trigger with entity " .. other_id)
end

-- The Player is kinematic: only dynamic bodies hitting it raise this.
function BotAI.OnCollisionEnter(self_id, other_id, contact)
    print("[Lua] 💥 OnCollisionEnter with entity " .. other_id .. ", impulse " .. contact.impulse)
end

return BotAI
