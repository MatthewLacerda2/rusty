-- project/scripts/bot_player.lua — DEV-ONLY bot-player (won't ship).
--
-- Attached to the Player, this script plays the game the way a human would: it drives
-- the WRITABLE Input from Update() — pressing the same W/A/S/D + arrow + SPACE keys
-- winit would inject — to navigate toward the enemy, aim the follow-camera at it, and
-- pull the trigger. It presses keys ONLY; the bundled player_controller turns those
-- presses into movement, camera follow, and the hitscan cast. So the bot exercises
-- the exact same control path a human does, validating that the de-hardcoded
-- player_controller.lua actually plays the demo. The match is a proximity demo: the
-- bot closes on the enemy and settles once it is within ARRIVE_RANGE.
--
-- Because one entity has one script slot, attaching the bot replaces the Player's
-- controller; the bot therefore loads the shared controller and runs its drive() each
-- frame, after injecting input — identical to the controller reading live input.
--
-- Determinism: it reads scene state + Time only, never the wall clock, so a headless
-- replay is identical every run.

local Bot = {}

-- The bundled default controller, shared verbatim so the bot drives the SAME
-- movement/camera/weapon code the human-attached Player uses.
local Controller = dofile("project/assets/scripts/player_controller.lua")

local ENEMY = "Enemy_1"
local SHOOT_RANGE = 14.0    -- start firing once within this distance
local ARRIVE_RANGE = 4.0    -- stop and settle once within this distance of the enemy
local AIM_TOLERANCE = 6.0   -- degrees of yaw error we tolerate before shooting
local SHOOT_KEY = "SPACE"   -- the trigger key the controller fires on (rising edge)
local SHOOT_COOLDOWN = 12   -- frames between shots (fire ~5x/second @ 60Hz)

-- Wrap a degree delta into (-180, 180].
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
    if enemy == nil or enemy == 0 then
        return
    end

    local px, py, pz = Transform.GetPosition(entity_id)
    local ex, ey, ez = Transform.GetPosition(enemy)
    local dx, dz = ex - px, ez - pz
    local dist = math.sqrt(dx * dx + dz * dz)

    -- Stop once we have closed to within ARRIVE_RANGE: release every key so the
    -- world settles. This is the proximity-demo win condition.
    if dist <= ARRIVE_RANGE then
        Input.Release("W")
        Input.Release("LEFT")
        Input.Release("RIGHT")
        Input.Release(SHOOT_KEY)
        return
    end

    -- The follow-camera's forward is (cos(yaw), _, sin(yaw)); the bearing to the enemy
    -- in that same convention is atan2(dz, dx). Steer the camera by pressing the arrow
    -- keys the controller maps to yaw — exactly what a human would hold.
    local want_yaw = math.atan2(dz, dx) * 180.0 / math.pi
    local yaw_err = wrap_deg(want_yaw - Camera.GetYaw())
    Input.Release("LEFT")
    Input.Release("RIGHT")
    if yaw_err > AIM_TOLERANCE then
        Input.Press("RIGHT")
    elseif yaw_err < -AIM_TOLERANCE then
        Input.Press("LEFT")
    end

    -- Close the distance: we only reach here while still beyond ARRIVE_RANGE
    -- (the arrival check above returns otherwise), so hold W to keep advancing.
    Input.Press("W")

    -- Pull the trigger when in range and on-target. Press SPACE for one frame so the
    -- controller sees a rising edge and fires; release it the rest of the cooldown so
    -- the next press re-triggers. The controller (not the bot) casts the hitscan.
    if Bot.cooldown > 0 then
        Bot.cooldown = Bot.cooldown - 1
        Input.Release(SHOOT_KEY)
    end
    if dist <= SHOOT_RANGE and math.abs(yaw_err) <= AIM_TOLERANCE and Bot.cooldown == 0 then
        Input.Press(SHOOT_KEY)
        Bot.cooldown = SHOOT_COOLDOWN
        Bot.shots = Bot.shots + 1
    end

    -- Run the shared controller with the input we just injected: it moves the Player,
    -- trails the camera, and fires on the SPACE rising edge.
    Controller.drive(Bot, entity_id, delta_time)
end

return Bot
