-- shooter.lua — the player: a first-person deathmatch fighter.
--
--   * Mouse looks (the cursor is locked), WASD walks through the
--     CharacterController at MOVE_SPEED, Space jumps, the script owns gravity.
--   * Left mouse fires while held (combat.lua's gun: spread, recoil, wallbangs);
--     G throws the smoke grenade THROW metres ahead, one per life.
--   * Dies at 0 health and respawns at the spawn farthest from everyone.

local Combat = require("combat")

local Shooter = {}

local MOVE_SPEED = 5.0
local GRAVITY = 20.0
local JUMP_SPEED = math.sqrt(2.0 * GRAVITY * 1.1) -- a 1.1 m jump, onto the crate
local LOOK = 0.12   -- degrees of turn per pixel of mouse motion
local THROW = 8.0   -- metres ahead the smoke lands

function Shooter.Start(id)
    Shooter.health = Combat.HEALTH
    Shooter.gun = Combat.gun({ rate = 0.1, spread = 0.004, kick = 0.01, recover = 0.15 })
    Shooter.smokes = 1
    Shooter.vy = 0
    Shooter.yaw, Shooter.pitch = 45, 0
    Input.SetCursorLocked(true)
    Input.SetCursorVisible(false)
end

local function warp(id, x, y, z)
    Transform.SetPosition(id, x, y, z)
    Shooter.vy = 0
    Shooter.smokes = 1
end

local function look(dt)
    local mx, my = Input.GetMouseDelta()
    Shooter.yaw = Shooter.yaw + mx * LOOK
    Shooter.pitch = math.max(-85, math.min(85, Shooter.pitch - my * LOOK))
    local yaw, pitch = math.rad(Shooter.yaw), math.rad(Shooter.pitch)
    return math.cos(yaw) * math.cos(pitch), math.sin(pitch), math.sin(yaw) * math.cos(pitch)
end

local function walk(id, fx, fz, dt)
    local len = math.sqrt(fx * fx + fz * fz)
    fx, fz = fx / len, fz / len
    local rx, rz = fz, -fx
    local mx, mz = 0, 0
    if Input.IsKeyDown("W") then mx, mz = mx + fx, mz + fz end
    if Input.IsKeyDown("S") then mx, mz = mx - fx, mz - fz end
    if Input.IsKeyDown("D") then mx, mz = mx - rx, mz - rz end
    if Input.IsKeyDown("A") then mx, mz = mx + rx, mz + rz end
    local m = math.sqrt(mx * mx + mz * mz)
    if m > 0 then mx, mz = mx / m * MOVE_SPEED * dt, mz / m * MOVE_SPEED * dt end
    if CharacterController.IsGrounded(id) then
        Shooter.vy = Input.GetKeyDown("SPACE") and JUMP_SPEED or -1.0
    else
        Shooter.vy = Shooter.vy - GRAVITY * dt
    end
    CharacterController.Move(id, mx, Shooter.vy * dt, mz)
end

function Shooter.Update(id, dt)
    local match = Combat.match()
    if (match ~= nil and match.over) or Combat.dead(Shooter, id, dt, warp) then return end
    Combat.tick(Shooter.gun, dt)
    local fx, fy, fz = look(dt)
    walk(id, fx, fz, dt)
    local x, y, z = Transform.GetPosition(id)
    if Input.IsKeyDown("Mouse0") then
        Combat.fire(Shooter.gun, id, x, y + Combat.EYE, z, fx, fy, fz)
    end
    if Input.GetKeyDown("G") and Shooter.smokes > 0 then
        Shooter.smokes = 0
        Combat.smoke(x + fx * THROW, y, z + fz * THROW)
    end
end

-- The camera sits at the eye once this tick's move has settled.
function Shooter.LateUpdate(id, dt)
    local x, y, z = Transform.GetPosition(id)
    Camera.SetPosition(x, y + Combat.EYE, z)
    Camera.SetYaw(Shooter.yaw)
    Camera.SetPitch(Shooter.pitch)
end

function Shooter.OnTakeDamage(id, amount, event)
    return Combat.hurt(Shooter, id, amount, event)
end

return Shooter
