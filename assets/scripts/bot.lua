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

function BotAI.Update(entity_id, delta_time)
    -- Fetch the player position from the engine scene system
    local player_id = Scene.FindEntityByName("Player")
    if player_id == 0 or player_id == nil then
        return
    end

    local pos_x, pos_y, pos_z = Transform.GetPosition(entity_id)
    local target_x, target_y, target_z = Transform.GetPosition(player_id)
    
    -- Dynamic path target tracking
    NavMeshAgent.SetTarget(entity_id, target_x, target_y, target_z)

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

function BotAI.Damage(entity_id, amount)
    BotAI.health = BotAI.health - amount
    print("[Lua] Bot took " .. amount .. " damage! Remaining HP: " .. BotAI.health)
    
    if BotAI.health <= 0.0 then
        Animator.Play(entity_id, "Death")
        print("[Lua] Bot is DEAD! Triggering death animation.")
    else
        Animator.Play(entity_id, "Hit")
    end
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
