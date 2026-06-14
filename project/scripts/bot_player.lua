-- project/scripts/bot_player.lua — DEV-ONLY bot-player (won't ship).
--
-- Attached to the Player, this script plays the game the way a human would: it drives
-- the WRITABLE Input from Update() — pressing the same W/A/S/D + arrow keys winit would
-- inject — to navigate toward the enemy and aim the follow-camera at it. The actual
-- trigger-pull is a hitscan: Input.Press("SPACE") is a winit-only signal the play loop
-- consumes from its own field, so the bot fires the laser through Physics.Shoot, the
-- same engine API the shoot button ultimately calls.
--
-- Determinism: it only reads scene state + Time, never the wall clock, so a headless
-- replay is identical every run.

local Bot = {}

local ENEMY = "Enemy_1"
local SHOOT_RANGE = 14.0    -- start firing once within this distance
local AIM_TOLERANCE = 6.0   -- degrees of yaw error we tolerate before shooting
local DAMAGE = 25.0         -- per-shot hitscan damage (matches the player's laser)
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
    print("[bot_player] online — hunting " .. ENEMY)
end

function Bot.Update(entity_id, delta_time)
    local enemy = Scene.FindEntityByName(ENEMY)
    if enemy == nil or enemy == 0 then
        return
    end

    -- Stop once the enemy is dead: release every key so the world settles.
    local hp = Health.Get(enemy)
    if hp <= 0.0 then
        Input.Release("W")
        Input.Release("LEFT")
        Input.Release("RIGHT")
        return
    end

    local px, py, pz = Transform.GetPosition(entity_id)
    local ex, ey, ez = Transform.GetPosition(enemy)
    local dx, dz = ex - px, ez - pz
    local dist = math.sqrt(dx * dx + dz * dz)

    -- The follow-camera's forward is (cos(yaw), _, sin(yaw)); the bearing to the enemy
    -- in that same convention is atan2(dz, dx). Steer the camera by pressing the arrow
    -- keys the play loop maps to yaw — exactly what a human would hold.
    local want_yaw = math.atan2(dz, dx) * 180.0 / math.pi
    local yaw_err = wrap_deg(want_yaw - Camera.GetYaw())
    Input.Release("LEFT")
    Input.Release("RIGHT")
    if yaw_err > AIM_TOLERANCE then
        Input.Press("RIGHT")
    elseif yaw_err < -AIM_TOLERANCE then
        Input.Press("LEFT")
    end

    -- Close the distance: hold W while we are still out of comfortable range.
    if dist > 4.0 then
        Input.Press("W")
    else
        Input.Release("W")
    end

    -- Fire when in range and roughly on-target. Shoot the real hitscan along the live
    -- camera ray (the same ray the shoot button casts), respecting a frame cooldown.
    if Bot.cooldown > 0 then
        Bot.cooldown = Bot.cooldown - 1
    end
    if dist <= SHOOT_RANGE and math.abs(yaw_err) <= AIM_TOLERANCE and Bot.cooldown == 0 then
        local cx, cy, cz = Camera.GetPosition()
        -- Reconstruct the camera forward from yaw/pitch, matching Camera::forward():
        -- (cos(yaw)cos(pitch), sin(pitch), sin(yaw)cos(pitch)).
        local yaw_r = Camera.GetYaw() * math.pi / 180.0
        local pitch_r = Camera.GetPitch() * math.pi / 180.0
        local cp = math.cos(pitch_r)
        local fx = math.cos(yaw_r) * cp
        local fy = math.sin(pitch_r)
        local fz = math.sin(yaw_r) * cp
        local hit, hit_id = Physics.Shoot(cx, cy, cz, fx, fy, fz, DAMAGE)
        Bot.cooldown = SHOOT_COOLDOWN
        Bot.shots = Bot.shots + 1
        if hit and hit_id == enemy then
            print("[bot_player] hit " .. ENEMY .. " (shot " .. Bot.shots .. ")")
        end
    end
end

return Bot
