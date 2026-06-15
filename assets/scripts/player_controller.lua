-- player_controller.lua — bundled DEFAULT player behaviour (ships with the engine).
--
-- This is game logic, not engine logic: it is a normal entity script the engine
-- happens to ship and attach to the Player, exactly like the enemy's bot.lua. The
-- engine play loop runs only systems; movement, the third-person camera, and the
-- weapon all live HERE, so a game can edit or replace them without touching Rust.
--
-- Reproduces what the engine's old `drive_player` + `hitscan` did, verbatim:
--   * WASD moves the Player along the camera's ground plane at MOVE_SPEED.
--   * Arrow keys turn the follow-camera (yaw/pitch), pitch clamped to +/-80.
--   * The camera trails the Player at -forward*FOLLOW_BACK + up*FOLLOW_UP.
--   * The shoot key fires a hitscan on its RISING edge (one shot per press) for
--     SHOOT_DAMAGE, casting Physics.Shoot from the camera along its forward.
--
-- Determinism: reads Input + Time only, never the wall clock, so a headless replay
-- is identical every run.

local PlayerController = {}

local MOVE_SPEED = 5.0      -- units/second of ground movement
local LOOK_SPEED = 90.0     -- degrees/second of camera turn
local PITCH_LIMIT = 80.0    -- clamp camera pitch to +/- this
local FOLLOW_BACK = 4.5     -- how far the camera trails behind the Player
local FOLLOW_UP = 1.5       -- how high above the Player the camera sits
local SHOOT_KEY = "SPACE"   -- the trigger key the windowed front-end maps Space to
local SHOOT_DAMAGE = 25.0   -- per-shot hitscan damage (the value lives in the SCRIPT)

-- Move the Player along the camera's ground plane from WASD.
local function move(entity_id, dt)
    local fx, fy, fz = Camera.GetForward()
    local rx, ry, rz = Camera.GetRight()
    local mx, mz = 0.0, 0.0
    if Input.IsKeyDown("W") then mx = mx + fx; mz = mz + fz end
    if Input.IsKeyDown("S") then mx = mx - fx; mz = mz - fz end
    if Input.IsKeyDown("D") then mx = mx + rx; mz = mz + rz end
    if Input.IsKeyDown("A") then mx = mx - rx; mz = mz - rz end
    local len = math.sqrt(mx * mx + mz * mz)
    if len > 0.0316 then -- sqrt(0.001), matching the old length_squared > 0.001 guard
        local step = MOVE_SPEED * dt / len
        local px, py, pz = Transform.GetPosition(entity_id)
        Transform.SetPosition(entity_id, px + mx * step, py, pz + mz * step)
    end
end

-- Turn the follow-camera from the arrow keys, then trail it behind the Player.
local function aim_camera(entity_id, dt)
    local look = LOOK_SPEED * dt
    local yaw = Camera.GetYaw()
    local pitch = Camera.GetPitch()
    if Input.IsKeyDown("LEFT") then yaw = yaw - look end
    if Input.IsKeyDown("RIGHT") then yaw = yaw + look end
    if Input.IsKeyDown("UP") then pitch = pitch + look end
    if Input.IsKeyDown("DOWN") then pitch = pitch - look end
    if pitch > PITCH_LIMIT then pitch = PITCH_LIMIT end
    if pitch < -PITCH_LIMIT then pitch = -PITCH_LIMIT end
    Camera.SetYaw(yaw)
    Camera.SetPitch(pitch)

    local fx, fy, fz = Camera.GetForward()
    local px, py, pz = Transform.GetPosition(entity_id)
    Camera.SetPosition(px - fx * FOLLOW_BACK, py - fy * FOLLOW_BACK + FOLLOW_UP, pz - fz * FOLLOW_BACK)
end

-- Fire a hitscan on the rising edge of the shoot key (one shot per press).
local function shoot(self)
    local down = Input.IsKeyDown(SHOOT_KEY)
    if down and not self.shoot_was_down then
        local cx, cy, cz = Camera.GetPosition()
        local fx, fy, fz = Camera.GetForward()
        Physics.Shoot(cx, cy, cz, fx, fy, fz, SHOOT_DAMAGE)
    end
    self.shoot_was_down = down
end

-- The reusable per-frame drive: movement + camera follow + weapon. The bot-player
-- calls this after injecting its own Input, so it shares the exact same controller.
function PlayerController.drive(self, entity_id, dt)
    move(entity_id, dt)
    aim_camera(entity_id, dt)
    shoot(self)
end

function PlayerController.Start(entity_id)
    PlayerController.shoot_was_down = false
end

function PlayerController.Update(entity_id, delta_time)
    PlayerController.drive(PlayerController, entity_id, delta_time)
end

return PlayerController
