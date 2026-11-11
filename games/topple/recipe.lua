-- games/topple/recipe.lua — a tower-toppling physics game (#2037): throw balls at
-- stacks of boxes and planks, at figures of boxes held together by joints, and at
-- explosive barrels. A level is cleared when every target (the red boxes, the
-- figures' torsos) has fallen off its platform. Built from primitives; run in
-- edit mode by `make game`, `...` is the command line's `--param` table.
--
-- Four levels of rising size, one scene each, chained by scripts/level.lua:
--   1 tower    one stack of boxes;
--   2 planks   a plank tower and a jointed figure;
--   3 barrels  a barrel among boxes and a figure, a far stack out of its reach;
--   4 pile     a pyramid `pile` boxes a side, its top layer the targets.

local p = { seed = 1, pile = 6, start = 1 }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("topple: unknown parameter '" .. key .. "'") end
    p[key] = value
end
local function whole(key, lo, hi)
    local v = p[key]
    if math.type(v) ~= "integer" or v < lo or v > hi then
        error(string.format("topple: %s is a whole number %d..%d, not %s", key, lo, hi, tostring(v)))
    end
end
whole("pile", 2, 16)
whole("start", 1, 4)

local TOP = 1.0   -- metres, the height of every platform's top
local BOX = 0.5   -- metres, a box's side

-- The art every level shares: Snyder textures and zimmer sounds, made once.
local function noise_map(name, scale, dark, light)
    return Texture.Bake({
        resolution = 64, seed = p.seed,
        nodes = {
            { id = "n", op = "noise", kind = "fbm", scale = scale, octaves = 4 },
            { id = "c", op = "color_ramp", inputs = { "n" },
              stops = { { pos = 0.0, color = dark }, { pos = 1.0, color = light } } },
        },
    }, "assets/textures/" .. name .. "_base_color.png", "base_color")
end
local function disc_map(name)
    return Texture.Bake({
        resolution = 64, seed = p.seed, output = "disc",
        nodes = {
            { id = "white", op = "constant", color = { 1, 1, 1, 1 } },
            { id = "shape", op = "shape", kind = "circle", size = { 0.9, 0.9 }, softness = 0.45 },
            { id = "disc", op = "combine_rgb", inputs = { "white", "white", "white", "shape" } },
        },
    }, "assets/textures/" .. name .. ".png", "base_color")
end
local maps = {
    wood = noise_map("wood", 9.0, { 0.32, 0.17, 0.06, 1 }, { 0.72, 0.48, 0.24, 1 }),
    stone = noise_map("stone", 5.0, { 0.3, 0.3, 0.32, 1 }, { 0.55, 0.55, 0.58, 1 }),
    grass = noise_map("grass", 12.0, { 0.16, 0.3, 0.1, 1 }, { 0.3, 0.48, 0.18, 1 }),
}
local puff = disc_map("puff")
local scorch = disc_map("scorch")
local function sound(name, kind, note, a, d, duration)
    Sound.Bake({
        source = kind == "noise" and { kind = "noise" }
            or { kind = "osc_stack", oscs = { { wave = kind, gain = 1.0 } } },
        amp = { a = a, d = d, s = 0.0, r = d / 2 },
    }, note, "assets/sounds/" .. name .. ".wav", { duration = duration, seed = p.seed })
end
sound("knock", "triangle", "C3", 0.001, 0.08, 0.15)  -- a ball striking something
sound("thud", "noise", "C2", 0.002, 0.12, 0.2)       -- a body landing hard
sound("boom", "noise", "C1", 0.004, 0.9, 1.2)        -- a barrel going off

-- Materials are a scene's, so each level defines them again.
local function materials()
    Material.DefineAsset("wood", { base_color_map = maps.wood, roughness = 0.8 })
    Material.DefineAsset("plank", { base_color_map = maps.wood, base_color = { 0.8, 0.7, 0.55 }, roughness = 0.8 })
    Material.DefineAsset("stone", { base_color_map = maps.stone, roughness = 0.9 })
    Material.DefineAsset("grass", { base_color_map = maps.grass, roughness = 1.0 })
    Material.DefineAsset("target", { base_color = { 0.85, 0.12, 0.1 }, roughness = 0.6 })
    Material.DefineAsset("cloth", { base_color = { 0.25, 0.35, 0.6 }, roughness = 0.9 })
    Material.DefineAsset("skin", { base_color = { 0.9, 0.7, 0.55 }, roughness = 0.7 })
    Material.DefineAsset("barrel", { base_color = { 0.8, 0.15, 0.05 }, metallic = 0.6, roughness = 0.4 })
    Material.DefineAsset("ball", { base_color = { 0.15, 0.15, 0.17 }, metallic = 0.3, roughness = 0.3 })
    Material.DefineAsset("scorch", { base_color = { 0.05, 0.04, 0.03 }, base_color_map = scorch, roughness = 1.0 })
end

-- A primitive of full size `sx, sy, sz` centred at `x, y, z`, with a collider
-- that fits it (#2051: a primitive gets none of its own).
local SHAPES = {
    Box = { kind = "Box", x = 1, y = 1, z = 1 },
    Sphere = { kind = "Sphere", radius = 0.5 },
    Cylinder = { kind = "Cylinder", radius = 0.5, height = 2 },
}
local bodies -- the level's group of dynamic bodies, which the scripts list
local function solid(name, kind, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity(name, kind)
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.Set(id, material)
    Scene.AddComponent(id, "Collider")
    Physics.SetColliderShape(id, SHAPES[kind])
    return id
end
local function static(name, x, y, z, sx, sy, sz, material)
    local id = solid(name, "Box", x, y, z, sx, sy, sz, material)
    Scene.SetStatic(id, true)
    return id
end
local function body(name, kind, x, y, z, sx, sy, sz, mass, material)
    local id = solid(name, kind, x, y, z, sx, sy, sz, material)
    Scene.SetParent(id, bodies)
    Scene.AddComponent(id, "RigidBody")
    Physics.SetMass(id, mass)
    return id
end

-- A box whose bottom sits at `y`: a target when its name starts "Target".
local function box(name, x, y, z, material)
    return body(name, "Box", x, y + BOX / 2, z, BOX, BOX, BOX, 1.0, material or "wood")
end
local function stack(x, z, n, y0)
    for k = 1, n do
        local top = k == n
        box(top and "Target" or "Box", x, (y0 or TOP) + (k - 1) * BOX, z, top and "target" or nil)
    end
end
-- A platform of `w` by `d` metres, its top at TOP, centred at `x, z`.
local function platform(x, z, w, d)
    static("Platform", x, TOP / 2, z, w, TOP, d, "stone")
end

-- A figure: six boxes, the head and limbs ball-jointed to the torso, standing
-- on the platform at `x, z`. Its joints' drives hold it up until a hard hit
-- turns them off (scripts/figure.lua): then it is a ragdoll.
local function figure(x, z)
    local group = Scene.CreateEntity("Figure")
    Scene.SetParent(group, bodies)
    local function part(name, px, py, pz, sx, sy, sz, mass, material)
        local id = body(name, "Box", x + px, TOP + py, z + pz, sx, sy, sz, mass, material)
        Scene.SetParent(id, group)
        Scene.AddScript(id, "assets/scripts/figure.lua")
        return id
    end
    local torso = part("TargetTorso", 0, 1.05, 0, 0.5, 0.7, 0.3, 4.0, "cloth")
    local function limb(name, px, py, pz, sx, sy, sz, mass, material, ay, swing)
        local id = part(name, px, py, pz, sx, sy, sz, mass, material)
        Scene.AddComponent(id, "Joint")
        Joint.SetKind(id, "Ball")
        Joint.SetConnectedBody(id, torso)
        Joint.SetAnchor(id, 0, ay, 0)                -- local: the end that meets the torso
        Joint.SetAxis(id, 0, 1, 0)
        Joint.SetUseLimits(id, true)
        Joint.SetLimits(id, -30, 30)
        Joint.SetSwingLimit(id, swing)
        Joint.SetSwing2Limit(id, swing)
        Joint.SetDriveStrength(id, 3000)
        Joint.SetDriveDamping(id, 110)
        return id
    end
    limb("Head", 0, 1.56, 0, 0.3, 0.3, 0.3, 1.0, "skin", -0.5, 40)
    limb("Arm", -0.34, 1.12, 0, 0.16, 0.56, 0.16, 0.6, "cloth", 0.5, 120)
    limb("Arm", 0.34, 1.12, 0, 0.16, 0.56, 0.16, 0.6, "cloth", 0.5, 120)
    limb("Leg", -0.13, 0.35, 0, 0.2, 0.7, 0.24, 1.5, "cloth", 0.5, 120)
    limb("Leg", 0.13, 0.35, 0, 0.2, 0.7, 0.24, 1.5, "cloth", 0.5, 120)
end

-- An explosive barrel standing on `y` (scripts/barrel.lua).
local function barrel(x, y, z)
    local id = body("Barrel", "Cylinder", x, y + 0.45, z, 0.6, 0.45, 0.6, 2.0, "barrel")
    Scene.AddScript(id, "assets/scripts/barrel.lua")
    return id
end

-- What every level has around its targets: ground, light, the camera that
-- throws, the effects, the HUD and the level's own script.
local function level(n, balls, eye, build)
    Scene.New()
    materials()
    static("Ground", 0, -0.1, 0, 60, 0.2, 60, "grass")
    bodies = Scene.CreateEntity("Bodies")
    build()

    local sun = Scene.CreateEntity("Sun", "DirectionalLight")
    Transform.SetRotation(sun, -55, 35, 0)
    local camera = Scene.CreateEntity("Player")
    Scene.AddComponent(camera, "Camera")
    Camera.SetPosition(eye[1], eye[2], eye[3])
    Camera.SetYaw(eye[4])
    Camera.SetPitch(eye[5])
    Scene.AddScript(camera, "assets/scripts/thrower.lua", { balls = balls })

    local function emitter(name, r, g, b, size, life, speed)
        local id = Scene.CreateEntity(name)
        Scene.AddComponent(id, "Particles")
        Particles.SetRate(id, 0)
        Particles.SetShape(id, "Sphere", { radius = 0.2 })
        Particles.SetLifetime(id, life / 2, life)
        Particles.SetSpeed(id, speed / 2, speed)
        Particles.SetSize(id, size / 2, size)
        Particles.SetColor(id, r, g, b, 1)
        Component.Set(id, "Particles", { texture = puff }) -- #2052: no texture draws a checker
    end
    emitter("Fire", 1.0, 0.6, 0.2, 0.9, 0.5, 6.0)
    emitter("Dust", 0.6, 0.55, 0.45, 0.3, 0.6, 1.5)

    local canvas = UI.Create("Canvas")
    Scene.SetName(canvas, "HUD")
    local function label(name, ay, y, size)
        local id = UI.Create("Text", canvas)
        Scene.SetName(id, name)
        RectTransform.SetAnchorMin(id, 0.5, ay)
        RectTransform.SetAnchorMax(id, 0.5, ay)
        RectTransform.SetPivot(id, 0.5, ay)
        RectTransform.SetAnchoredPosition(id, 0, y)
        RectTransform.SetSizeDelta(id, 900, size * 1.5)
        Text.SetFontSize(id, size)
        Text.SetAlignment(id, "MiddleCenter")
        Text.SetText(id, "")
    end
    label("Status", 1, -30, 28)
    label("Banner", 0.5, 0, 56)

    local next_scene = n < 4 and string.format("assets/scenes/level_%d.scene", n + 1) or ""
    local manager = Scene.CreateEntity("Level")
    Scene.AddScript(manager, "assets/scripts/level.lua", { number = n, top = TOP, next = next_scene })
    Scene.Save(string.format("assets/scenes/level_%d.scene", n))
end

-- The camera stands on -Z looking along +Z: yaw 90 is +Z (Camera.SetYaw).
level(1, 5, { 0, 2.2, -7, 90, -5 }, function()
    platform(0, 0, 2, 2)
    stack(0, 0, 5)
end)

level(2, 6, { 0, 2.4, -8, 90, -5 }, function()
    platform(0, 0, 5, 2)
    -- A plank tower, Jenga style: layers of two planks, crossing each other.
    local PL, PH, PW = 1.2, 0.2, 0.3
    for k = 0, 4 do
        local y = TOP + PH / 2 + k * PH
        for _, s in ipairs({ -0.4, 0.4 }) do
            if k % 2 == 0 then
                body("Plank", "Box", -1.2, y, s, PL, PH, PW, 1.2, "plank")
            else
                body("Plank", "Box", -1.2 + s, y, 0, PW, PH, PL, 1.2, "plank")
            end
        end
    end
    box("Target", -1.2, TOP + 5 * PH, 0, "target")
    figure(1.2, 0)
end)

level(3, 4, { 0, 2.6, -9, 90, -6 }, function()
    platform(0, 0, 6, 2.5)
    barrel(0, TOP, 0)
    stack(-1.3, 0, 2)
    stack(1.3, 0, 2)
    figure(0, 0.9)
    -- Out of the blast's 3.5 m reach: a stack on a platform of its own.
    platform(7, 0, 2, 2)
    stack(7, 0, 3)
end)

level(4, 10, { 0, 3.5, -4 - p.pile, 90, -12 }, function()
    local side = p.pile * BOX + 1.5
    platform(0, 0, side, side)
    for layer = 0, p.pile - 1 do
        local count = p.pile - layer
        for i = 0, count - 1 do
            for j = 0, count - 1 do
                local x = (i - (count - 1) / 2) * BOX
                local z = (j - (count - 1) / 2) * BOX
                local top = layer >= p.pile - 2
                box(top and "Target" or "Box", x, TOP + layer * BOX, z, top and "target" or nil)
            end
        end
    end
    barrel(-side / 2 + 0.4, TOP, -side / 2 + 0.4)
    barrel(side / 2 - 0.4, TOP, -side / 2 + 0.4)
end)

Application.SetStartupScene(string.format("assets/scenes/level_%d.scene", p.start))
