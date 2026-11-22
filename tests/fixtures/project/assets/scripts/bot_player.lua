-- DEV-ONLY bot-player: presses keys through the writable Input. Attaching it clears the Player's
-- scripts, so it runs the bundled player_controller's drive() itself each frame, after injecting
-- input. Reads scene state + Time only: deterministic.

local Bot = {}

local Controller = dofile("assets/scripts/player_controller.lua")

local ENEMY = "Enemy_1"
local SHOOT_RANGE = 14.0    -- metres
local ARRIVE_RANGE = 4.0    -- metres; play_session.lua wins at the same distance: change both
local AIM_TOLERANCE = 6.0   -- degrees of yaw error we tolerate before shooting
local SHOOT_KEY = "Mouse0"
local SHOOT_COOLDOWN = 12   -- frames between shots

local function wrap_deg(d)
    while d > 180.0 do d = d - 360.0 end
    while d <= -180.0 do d = d + 360.0 end
    return d
end

function Bot.Start(entity_id)
    Bot.cooldown = 0
    Bot.shots = 0
    Controller.Start(entity_id)
    print("[bot_player] online — hunting " .. ENEMY)
end

function Bot.Update(entity_id, delta_time)
    local enemy = Scene.FindEntityByName(ENEMY)
    if enemy == nil then
        return
    end

    local px, py, pz = Transform.GetPosition(entity_id)
    local ex, ey, ez = Transform.GetPosition(enemy)
    local dx, dz = ex - px, ez - pz
    local dist = math.sqrt(dx * dx + dz * dz)

    if dist <= ARRIVE_RANGE then
        Input.Release("W")
        Input.Release("LEFT")
        Input.Release("RIGHT")
        Input.Release(SHOOT_KEY)
        return
    end

    -- Camera forward is (cos(yaw), _, sin(yaw)); the bearing to the enemy in that convention is
    -- atan2(dz, dx).
    local want_yaw = math.atan2(dz, dx) * 180.0 / math.pi
    local yaw_err = wrap_deg(want_yaw - Camera.GetYaw())
    Input.Release("LEFT")
    Input.Release("RIGHT")
    if yaw_err > AIM_TOLERANCE then
        Input.Press("RIGHT")
    elseif yaw_err < -AIM_TOLERANCE then
        Input.Press("LEFT")
    end

    Input.Press("W")

    -- Mouse0 for one frame gives the controller a rising edge; release it for the rest of the
    -- cooldown.
    if Bot.cooldown > 0 then
        Bot.cooldown = Bot.cooldown - 1
        Input.Release(SHOOT_KEY)
    end
    if dist <= SHOOT_RANGE and math.abs(yaw_err) <= AIM_TOLERANCE and Bot.cooldown == 0 then
        Input.Press(SHOOT_KEY)
        Bot.cooldown = SHOOT_COOLDOWN
        Bot.shots = Bot.shots + 1
    end

    Controller.drive(Bot, entity_id, delta_time)
end

return Bot
