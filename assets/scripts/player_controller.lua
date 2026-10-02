-- player_controller.lua — bundled DEFAULT player behaviour (ships with the engine).
--
-- This is game logic, not engine logic: it is a normal entity script the engine
-- happens to ship and attach to the Player, exactly like the enemy's bot.lua. The
-- engine play loop runs only systems; movement, the third-person camera, and the
-- weapon all live HERE, so a game can edit or replace them without touching Rust.
--
-- Reproduces what the engine's old `drive_player` + `hitscan` did, verbatim:
--   * WASD moves the Player along the camera's ground plane at MOVE_SPEED, through
--     its CharacterController (#451): walls block it, steps are climbed, and the
--     script applies its own GRAVITY, since Move applies none.
--   * Arrow keys turn the follow-camera (yaw/pitch), pitch clamped to +/-80.
--   * The camera trails the Player at -forward*FOLLOW_BACK + up*FOLLOW_UP.
--   * The shoot key casts a hitscan on its RISING edge (one cast per press),
--     firing Physics.Raycast from the camera along its forward.
--   * Start frames the camera behind the Player (yaw 90, pitch -10) — this used to
--     be an engine-side snap keyed on the name "Player" (#450).
--
-- Determinism: reads Input + Time only, never the wall clock, so a headless replay
-- is identical every run.

local PlayerController = {}

local MOVE_SPEED = 5.0      -- units/second of ground movement
local GRAVITY = 20.0        -- units/second² the Player falls at
local GROUND_STICK = -1.0   -- vertical speed while grounded, keeping it pressed down
local LOOK_SPEED = 90.0     -- degrees/second of camera turn
local PITCH_LIMIT = 80.0    -- clamp camera pitch to +/- this
local FOLLOW_BACK = 4.5     -- how far the camera trails behind the Player
local FOLLOW_UP = 1.5       -- how high above the Player the camera sits
local SHOOT_KEY = "SPACE"   -- the trigger key the windowed front-end maps Space to
local START_YAW = 90.0      -- initial camera yaw: looking down +Z, across the arena
local START_PITCH = -10.0   -- initial camera pitch: tilted slightly down
local START_BACK = 4.5      -- initial camera offset behind the Player (along -Z)
local START_UP = 1.5        -- initial camera height above the Player

-- Move the Player along the camera's ground plane from WASD, falling under GRAVITY.
local function move(self, entity_id, dt)
    local fx, fy, fz = Camera.GetForward()
    local rx, ry, rz = Camera.GetRight()
    local mx, mz = 0.0, 0.0
    if Input.IsKeyDown("W") then mx = mx + fx; mz = mz + fz end
    if Input.IsKeyDown("S") then mx = mx - fx; mz = mz - fz end
    if Input.IsKeyDown("D") then mx = mx + rx; mz = mz + rz end
    if Input.IsKeyDown("A") then mx = mx - rx; mz = mz - rz end
    local len = math.sqrt(mx * mx + mz * mz)
    local dx, dz = 0.0, 0.0
    if len > 0.0316 then -- sqrt(0.001), matching the old length_squared > 0.001 guard
        local step = MOVE_SPEED * dt / len
        dx, dz = mx * step, mz * step
    end
    local vy = self.vy or 0.0
    if CharacterController.IsGrounded(entity_id) and vy < 0.0 then
        vy = GROUND_STICK
    end
    vy = vy - GRAVITY * dt
    CharacterController.Move(entity_id, dx, vy * dt, dz)
    self.vy = vy
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

-- Cast a hitscan on the rising edge of the shoot key (one cast per press).
-- Passes entity_id as the ignore id so the cast can't hit the shooter itself —
-- the "don't hit the Player" rule lives HERE, not as a name check in the engine.
local function shoot(self, entity_id)
    local down = Input.IsKeyDown(SHOOT_KEY)
    if down and not self.shoot_was_down then
        local cx, cy, cz = Camera.GetPosition()
        local fx, fy, fz = Camera.GetForward()
        Physics.Raycast(cx, cy, cz, fx, fy, fz, entity_id)
    end
    self.shoot_was_down = down
end

-- The reusable per-frame drive: movement + camera follow + weapon. The bot-player
-- calls this after injecting its own Input, so it shares the exact same controller.
function PlayerController.drive(self, entity_id, dt)
    move(self, entity_id, dt)
    aim_camera(entity_id, dt)
    shoot(self, entity_id)
end

-- Frame the follow-camera behind the Player when play starts. This is the demo's
-- choice, not the engine's: the engine never moves the camera on entering Play.
local function frame_camera(entity_id)
    local px, py, pz = Transform.GetPosition(entity_id)
    Camera.SetPosition(px, py + START_UP, pz - START_BACK)
    Camera.SetYaw(START_YAW)
    Camera.SetPitch(START_PITCH)
end

function PlayerController.Start(entity_id)
    PlayerController.shoot_was_down = false
    PlayerController.vy = 0.0
    frame_camera(entity_id)
end

function PlayerController.Update(entity_id, delta_time)
    PlayerController.drive(PlayerController, entity_id, delta_time)
end

return PlayerController
