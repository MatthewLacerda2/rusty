-- The player: a first-person deathmatch fighter.
--
--   * Mouse looks (the cursor is locked), WASD walks through the
--     CharacterController at MOVE_SPEED, Space jumps, the script owns gravity.
--   * Left mouse fires (combat.lua's gun), a magazine at a time (R reloads);
--     G throws the smoke grenade THROW metres ahead. Feeds the shared HUD.
--   * Dies at 0 health and respawns at the spawn farthest from everyone.

local Combat = require("combat")

local Shooter = {}
local shown = {}

local MOVE_SPEED = 5.0
local GRAVITY = 20.0
local JUMP_SPEED = math.sqrt(2.0 * GRAVITY * 1.1) -- a 1.1 m jump, onto the crate
local LOOK = 0.12   -- degrees of turn per pixel of mouse motion
local THROW = 8.0   -- metres ahead the smoke lands
local MAG = 30
local RESERVE = 90
local RELOAD = 1.5  -- s

function Shooter.Start(id)
    Combat.start()
    Shooter.health = Combat.HEALTH
    Shooter.gun = Combat.gun({ rate = 0.1, spread = 0.004, kick = 0.01, recover = 0.15 })
    Shooter.smokes = 1
    Shooter.mag, Shooter.reserve, Shooter.reloading = MAG, RESERVE, 0
    Shooter.hits, Shooter.hud, shown = {}, nil, {}
    Shooter.vy = 0
    Shooter.yaw, Shooter.pitch = 45, 0
    Input.SetCursorLocked(true)
    Input.SetCursorVisible(false)
end

local function warp(id, x, y, z)
    Transform.SetPosition(id, x, y, z)
    Shooter.vy = 0
    Shooter.smokes = 1
    Shooter.mag, Shooter.reserve, Shooter.reloading = MAG, RESERVE, 0
end

local function look(dt)
    local mx, my = Input.GetMouseDelta()
    local turn = LOOK * (Shooter.look_scale or 1)
    if Shooter.invert_look then my = -my end
    Shooter.yaw = Shooter.yaw + mx * turn
    Shooter.pitch = math.max(-85, math.min(85, Shooter.pitch - my * turn))
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

local function reload(dt)
    if Shooter.reloading > 0 then
        Shooter.reloading = Shooter.reloading - dt
        if Shooter.reloading <= 0 then
            local take = math.min(MAG - Shooter.mag, Shooter.reserve)
            Shooter.mag, Shooter.reserve = Shooter.mag + take, Shooter.reserve - take
        end
    elseif Shooter.reserve > 0 and Shooter.mag < MAG
        and (Shooter.mag == 0 or Input.GetKeyDown("R")) then
        Shooter.reloading = RELOAD
    end
end

-- The HUD may load after this.
local function hud_of()
    Shooter.hud = Shooter.hud or Scene.GetScript(Scene.FindEntityByName("HUD"), "hud")
    return Shooter.hud
end

local function fired()
    local hud = hud_of()
    if hud == nil then return end
    hud.Fired()
    for _, hit in ipairs(Shooter.hits) do hud.Hit(hit.x, hit.y, hit.z, hit.amount) end
end

local function feed()
    local hud = hud_of()
    if hud == nil then return end
    if shown.health ~= Shooter.health then
        shown.health = Shooter.health
        hud.SetHealth(Shooter.health, Combat.HEALTH)
    end
    if shown.mag ~= Shooter.mag or shown.reserve ~= Shooter.reserve then
        shown.mag, shown.reserve = Shooter.mag, Shooter.reserve
        hud.SetAmmo(Shooter.mag, Shooter.reserve)
    end
    local spread = Shooter.gun.spread + Shooter.gun.recoil
    if shown.spread ~= spread then
        shown.spread = spread
        hud.SetSpread(spread)
    end
end

function Shooter.Update(id, dt)
    if Shooter.paused then return end
    local match = Combat.match()
    if (match ~= nil and match.over) or Combat.dead(Shooter, id, dt, warp) then
        feed()
        return
    end
    Combat.tick(Shooter.gun, dt)
    local fx, fy, fz = look(dt)
    walk(id, fx, fz, dt)
    local x, y, z = Transform.GetPosition(id)
    reload(dt)
    if Input.IsKeyDown("Mouse0") and Shooter.mag > 0 and Shooter.reloading <= 0
        and Combat.fire(Shooter.gun, id, x, y + Combat.EYE, z, fx, fy, fz, Shooter.hits) then
        Shooter.mag = Shooter.mag - 1
        fired()
    end
    feed()
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
    if hud_of() and Shooter.health > 0 then Shooter.hud.Hurt(amount) end
    return Combat.hurt(Shooter, id, amount, event)
end

return Shooter
