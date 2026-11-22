-- Bundled default player behaviour: an ordinary script a game can replace.
-- Reads Input + Time only, so a headless replay is identical.

local PlayerController = {}

local MOVE_SPEED = 5.0      -- units/second
local GRAVITY = 20.0        -- units/second²
local JUMP_HEIGHT = 1.1     -- metres
local JUMP_SPEED = math.sqrt(2.0 * GRAVITY * JUMP_HEIGHT)
local GROUND_STICK = -1.0
local LOOK_SPEED = 90.0     -- degrees/second
local PITCH_LIMIT = 80.0    -- degrees
local FOLLOW_BACK = 4.5
local FOLLOW_UP = 1.5
local SHOT_RANGE = 100.0    -- metres
local SHOT_DAMAGE = 25.0    -- a bot has 100
local JUMP_KEY = "SPACE"
local SHOOT_KEY = "Mouse0"
local START_YAW = 90.0
local START_PITCH = -10.0
local START_BACK = 4.5
local START_UP = 1.5

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
    local grounded = CharacterController.IsGrounded(entity_id)
    if grounded and vy < 0.0 then
        vy = GROUND_STICK
    end
    if grounded and Input.GetKeyDown(JUMP_KEY) then
        vy = JUMP_SPEED
    end
    vy = vy - GRAVITY * dt
    CharacterController.Move(entity_id, dx, vy * dt, dz)
    self.vy = vy
end

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

-- Hits on the Player's own
-- hierarchy are skipped here, not by a name check in the engine.
local function shoot(self, entity_id)
    local down = Input.IsKeyDown(SHOOT_KEY)
    if down and not self.shoot_was_down then
        local cx, cy, cz = Camera.GetPosition()
        local fx, fy, fz = Camera.GetForward()
        -- Triggers (pickups, zones) don't stop a bullet: query_triggers = false.
        for _, hit in ipairs(Physics.RaycastAll(cx, cy, cz, fx, fy, fz, SHOT_RANGE, nil, false)) do
            if hit.root ~= entity_id then
                Damage.ApplyPoint(hit, SHOT_DAMAGE, fx, fy, fz,
                    { type = "bullet", instigator = entity_id })
                break
            end
        end
    end
    self.shoot_was_down = down
end

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
