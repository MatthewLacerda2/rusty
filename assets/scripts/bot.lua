-- AI-generated bot.lua script expected by the engine
local BotAI = {}

BotAI.health = 100.0

function BotAI.Start(entity_id)
    Transform.SetPosition(entity_id, 8.0, 1.0, 8.0)
    Animator.Play(entity_id, "Walk")
    print("[Lua] Bot initialized at position (8, 1, 8)")
end

function BotAI.Update(entity_id, delta_time)
    -- Fetch the player position from the engine scene system
    local player_id = Scene.FindEntityByName("Player")
    if player_id == 0 or player_id == nil then
        return
    end

    local pos_x, pos_y, pos_z = Transform.GetPosition(entity_id)
    local target_x, target_y, target_z = Transform.GetPosition(player_id)
    
    -- Request path array from the engine's baked navigation system
    local next_x, next_y, next_z = Navigation.GetNextPathStep(pos_x, pos_y, pos_z, target_x, target_y, target_z)
    
    -- Interpolate towards the next step
    Transform.MoveTowards(entity_id, next_x, next_y, next_z, 3.0 * delta_time)

    -- Spin the entity dynamically to look in direction of player
    let_dx = target_x - pos_x
    let_dz = target_z - pos_z
    local angle = math.atan2(let_dx, let_dz) * (180.0 / math.pi)
    Transform.SetRotation(entity_id, 0.0, angle, 0.0)
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
