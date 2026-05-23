-- AI-generated bot.lua script expected by the engine
local BotAI = {}

BotAI.health = 100.0

function BotAI.Start(entity_id)
    Transform.SetPosition(entity_id, 8.0, 1.0, 8.0)
    NavMeshAgent.SetActive(entity_id, true)
    NavMeshAgent.SetSpeed(entity_id, 3.5)
    NavMeshAgent.SetAcceleration(entity_id, 8.0)
    NavMeshAgent.SetStoppingDistance(entity_id, 2.5) -- Stop 2.5 units away from player
    Animator.Play(entity_id, "Walk")
    print("[Lua] Bot initialized at position (8, 1, 8) with NavMeshAgent")
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

    -- Dynamic rotation towards movement velocity direction and clean animation transition
    local vx, vy, vz = NavMeshAgent.GetVelocity(entity_id)
    let_speed_sq = vx * vx + vz * vz
    if let_speed_sq > 0.01 then
        local angle = math.atan2(vx, vz) * (180.0 / math.pi)
        Transform.SetRotation(entity_id, 0.0, angle, 0.0)
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

-- OnTrigger callback hook triggered when this entity overlaps with a trigger collider
function BotAI.OnTrigger(self_id, other_id)
    print("[Lua] 🟢 OnTrigger overlap event! Entity " .. self_id .. " intersected trigger with entity " .. other_id)
end

return BotAI
