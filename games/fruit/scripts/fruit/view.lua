-- Never decides the game; `fruit.lua` does.

local View = {}

View.LAYER = 8          -- the Fruit layer: collides with nothing (the recipe sets it)
local SPLIT_SPEED = 2.5 -- how fast the halves part, metres per second
local FLASH_PEAK = 80.0 -- the flash light's intensity the tick a bomb goes off
local FLASH_DECAY = 0.92
local SOUND_AT = { 0, 2, 8 }

View.KINDS = {
    apple = { shape = "Sphere", scale = { 0.8, 0.8, 0.8 }, juice = { 1.0, 0.9, 0.6 } },
    orange = { shape = "Sphere", scale = { 0.9, 0.9, 0.9 }, juice = { 1.0, 0.6, 0.1 } },
    lime = { shape = "Sphere", scale = { 0.6, 0.6, 0.6 }, juice = { 0.6, 0.95, 0.3 } },
    banana = { shape = "Capsule", scale = { 0.4, 0.6, 0.4 }, juice = { 1.0, 0.95, 0.7 } },
    melon = { shape = "Sphere", scale = { 1.3, 1.3, 1.3 }, juice = { 0.95, 0.15, 0.2 } },
    bomb = { shape = "Sphere", scale = { 0.85, 0.85, 0.85 } },
}
View.FRUIT = { "apple", "orange", "lime", "banana", "melon" }

local function body(id)
    Scene.AddComponent(id, "Collider") -- a primitive gets none of its own yet
    Scene.AddComponent(id, "RigidBody")
    Layers.SetLayer(id, View.LAYER)
end

function View.radius(kind)
    local s = View.KINDS[kind].scale
    local widest = math.max(s[1], s[3])
    if View.KINDS[kind].shape == "Capsule" then widest = math.max(widest, 2 * s[2]) end
    return widest / 2
end

function View.throw(kind, x, y, z, vx, vy, spin)
    local k = View.KINDS[kind]
    local id = Scene.CreateEntity(kind, k.shape)
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, k.scale[1], k.scale[2], k.scale[3])
    Material.Set(id, kind == "bomb" and "bomb" or kind .. "_skin")
    body(id)
    if kind == "bomb" then
        local fuse = Scene.CreateEntity("Fuse", "Sphere")
        Scene.SetParent(fuse, id)
        Transform.SetLocalPosition(fuse, 0, 0.55, 0)
        Transform.SetLocalScale(fuse, 0.2, 0.2, 0.2)
        Material.Set(fuse, "fuse")
    end
    Physics.SetVelocity(id, vx, vy, 0)
    Physics.SetAngularVelocity(id, spin, spin * 0.5, spin * 0.3)
    return id
end

-- A half's skin is the fruit squashed along the cut's normal (a lens), its flesh
-- a disc across the cut: a real hemisphere needs a runtime mesh the API cannot make yet.
local function half(kind, x, y, z, nx, ny, side, vx, vy)
    local k = View.KINDS[kind]
    local d = math.max(k.scale[1], k.scale[2])
    local id = Scene.CreateEntity(kind .. "_half", "Sphere")
    Transform.SetPosition(id, x + nx * side * d / 4, y + ny * side * d / 4, z)
    -- The lens's local Y is the cut's normal: turn +Y onto (nx, ny) about Z.
    Transform.SetRotation(id, 0, 0, math.deg(math.atan(-nx, ny)))
    Transform.SetLocalScale(id, d, d / 2, d)
    Material.Set(id, kind .. "_skin")
    body(id)
    local cut = Scene.CreateEntity("Flesh", "Cylinder")
    Scene.SetParent(cut, id)
    Transform.SetLocalPosition(cut, 0, -side * 0.02, 0)
    Transform.SetLocalScale(cut, 1.04, 0.02, 1.04)
    Material.Set(cut, kind .. "_flesh")
    Physics.SetVelocity(id, vx + nx * side * SPLIT_SPEED, vy + ny * side * SPLIT_SPEED, 0)
    Physics.SetAngularVelocity(id, 0, 0, side * 3)
    return id
end

function View.split(id, kind, dx, dy, juice)
    local x, y, z = Transform.GetPosition(id)
    local vx, vy = Physics.GetVelocity(id)
    Scene.DestroyEntity(id)
    local len = math.sqrt(dx * dx + dy * dy)
    if len < 1e-6 then dx, dy, len = 1, 0, 1 end
    local nx, ny = -dy / len, dx / len
    local c = View.KINDS[kind].juice
    Transform.SetPosition(juice, x, y, z)
    Particles.SetColor(juice, c[1], c[2], c[3], 1)
    Particles.Burst(juice)
    View.sound("slice", x, y)
    return { half(kind, x, y, z, nx, ny, 1, vx, vy), half(kind, x, y, z, nx, ny, -1, vx, vy) }
end

function View.explode(id, blast)
    local x, y, z = Transform.GetPosition(id)
    Scene.DestroyEntity(id)
    Transform.SetPosition(blast, x, y, z)
    Particles.SetColor(blast, 1.0, 0.55, 0.15, 1)
    Particles.Burst(blast)
    View.sound("bomb", x, y)
    return FLASH_PEAK
end

function View.flash(light, intensity)
    Light.SetIntensity(light, intensity)
    return intensity * FLASH_DECAY
end

function View.sound(name, x, y)
    Audio.PlayAt("assets/sounds/" .. name .. ".wav", x or SOUND_AT[1], y or SOUND_AT[2], SOUND_AT[3], 0.8)
end

function View.find()
    return {
        score = Scene.FindEntityByName("ScoreText"),
        combo = Scene.FindEntityByName("ComboText"),
        lives = Scene.FindEntityByName("LivesText"),
        over = Scene.FindChild(Scene.FindEntityByName("HUD"), "GameOver"), -- inactive until the end
        juice = Scene.FindEntityByName("Juice"),
        blast = Scene.FindEntityByName("Blast"),
        flash = Scene.FindEntityByName("Flash"),
    }
end

function View.show(hud, score, combo, lives, over)
    Text.SetText(hud.score, tostring(score))
    Text.SetText(hud.combo, combo >= 3 and (combo .. " fruit combo!") or "")
    Text.SetText(hud.lives, string.rep("X ", lives))
    Scene.SetActive(hud.over, over)
end

return View
