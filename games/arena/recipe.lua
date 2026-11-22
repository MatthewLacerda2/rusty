-- A grid of 12 m rooms joined by doorways; `size = "dust2"` is a 5 × 5 grid of
-- blocks (120 m a side, Dust2's footprint).

local p = { size = "default", seed = 1, bots = 4, kills = 10, time = 120, bake = false }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("arena: unknown parameter '" .. key .. "'") end
    p[key] = value
end
local BLOCKS = ({ default = 1, dust2 = 5 })[p.size]
if BLOCKS == nil then error("arena: size is 'default' or 'dust2', not '" .. tostring(p.size) .. "'") end

local ROOM = 12      -- m
local WALL_H = 4     -- m
local WALL_T = 0.4   -- m, thick enough to stop a bullet
local DOOR = 2.4     -- m
local UPPER = 3      -- m
local ROOMS = 2 * BLOCKS
local SIDE = ROOMS * ROOM

Scene.New()
local FIGHTER = 8
local SMOKE = 9
Layers.SetName(FIGHTER, "Fighter")
Layers.SetName(SMOKE, "Smoke")
for layer = 0, 31 do Physics.IgnoreLayerCollision(SMOKE, layer) end

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
Material.DefineAsset("concrete", { base_color_map = noise_map("concrete", 6.0,
    { 0.35, 0.34, 0.32, 1 }, { 0.6, 0.58, 0.55, 1 }), roughness = 0.9 })
Material.DefineAsset("wood", { base_color_map = noise_map("wood", 10.0,
    { 0.3, 0.16, 0.06, 1 }, { 0.7, 0.45, 0.22, 1 }), roughness = 0.8 })
Material.DefineAsset("floor", { base_color = { 0.42, 0.4, 0.36 }, roughness = 0.95 })
Material.DefineAsset("snow", { base_color = { 0.95, 0.95, 0.97 }, roughness = 0.7 })
Material.DefineAsset("carrot", { base_color = { 0.95, 0.45, 0.1 }, roughness = 0.6 })
Material.DefineAsset("stick", { base_color = { 0.2, 0.15, 0.1 }, roughness = 0.8 })
Material.DefineAsset("steel", { base_color = { 0.5, 0.52, 0.55 }, metallic = 1.0, roughness = 0.4 })

local function box(name, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity(name, "Box")
    Transform.SetPosition(id, x, y + sy / 2, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.Set(id, material)
    Scene.AddComponent(id, "Collider")
    Physics.SetColliderShape(id, { kind = "Box", x = 1, y = 1, z = 1 })
    Scene.SetStatic(id, true)
    return id
end

box("Ground", SIDE / 2, -0.2, SIDE / 2, SIDE + WALL_T, 0.2, SIDE + WALL_T, "floor")
box("Wall_S", SIDE / 2, 0, 0, SIDE + WALL_T, WALL_H, WALL_T, "concrete")
box("Wall_N", SIDE / 2, 0, SIDE, SIDE + WALL_T, WALL_H, WALL_T, "concrete")
box("Wall_W", 0, 0, SIDE / 2, WALL_T, WALL_H, SIDE, "concrete")
box("Wall_E", SIDE, 0, SIDE / 2, WALL_T, WALL_H, SIDE, "concrete")

local PIECE = (ROOM - DOOR) / 2
for line = 1, ROOMS - 1 do
    for room = 0, ROOMS - 1 do
        local a, b = room * ROOM + PIECE / 2, (room + 1) * ROOM - PIECE / 2
        local at = line * ROOM
        box("Wall", at, 0, a, WALL_T, WALL_H, PIECE, "concrete")
        box("Wall", at, 0, b, WALL_T, WALL_H, PIECE, "concrete")
        box("Wall", a, 0, at, PIECE, WALL_H, WALL_T, "concrete")
        box("Wall", b, 0, at, PIECE, WALL_H, WALL_T, "concrete")
    end
end

local spawns, waypoints = {}, {}
local groups = { [spawns] = Scene.CreateEntity("Spawns"), [waypoints] = Scene.CreateEntity("Waypoints") }
local function marker(list, name, x, y, z)
    local id = Scene.CreateEntity(name)
    Scene.SetParent(id, groups[list])
    Transform.SetPosition(id, x, y, z)
    list[#list + 1] = id
end

local function block(ox, oz, n)
    local tag = "_" .. n
    -- Room A's 10 cm wall lets a shot through (combat.lua's `RESIST`).
    box("ThinWall" .. tag, ox + 6, 0, oz + 6, 0.1, 2.5, 3, "wood")
    local ux0, ux1 = ox + 18, ox + 24 - WALL_T / 2
    box("Upper" .. tag, (ux0 + ux1) / 2, UPPER - 0.2, oz + 6, ux1 - ux0, 0.2, ROOM - WALL_T, "wood")
    -- The staircase up its south side, against the wall: 12 steps of 25 cm,
    -- under the navmesh's 30 cm MaxStep, each solid to the ground.
    local steps, tread = 12, 0.35
    for k = 1, steps do
        local x0 = ux0 - (steps - k + 1) * tread
        box("Step" .. tag, (x0 + ux0) / 2, 0, oz + 1.4, ux0 - x0, k * UPPER / steps, 2, "concrete")
    end
    local ladder = box("Ladder" .. tag, ux0 - 0.05, 0, oz + 10, 0.1, UPPER, 0.6, "steel")
    Scene.RemoveComponent(ladder, "Collider")
    local link = Scene.CreateEntity("LadderLink" .. tag)
    Transform.SetPosition(link, ux0 - 0.7, 0, oz + 10)
    Scene.AddComponent(link, "OffMeshLink")
    OffMeshLink.SetStart(link, 0, 0, 0)
    OffMeshLink.SetEnd(link, 1.4, UPPER, 0)
    OffMeshLink.SetBidirectional(link, true)
    box("Crate" .. tag, ox + 6, 0, oz + 18, 1.5, 1, 1.5, "wood")
    box("Cover" .. tag, ox + 16, 0, oz + 16, 1.2, 1.4, 2.4, "concrete")
    box("Cover" .. tag, ox + 20, 0, oz + 20, 2.4, 1.4, 1.2, "concrete")

    marker(spawns, "Spawn" .. tag .. "_A", ox + 3, 0, oz + 3)
    marker(spawns, "Spawn" .. tag .. "_B", ox + 15, 0, oz + 9)
    marker(spawns, "Spawn" .. tag .. "_C", ox + 3, 0, oz + 21)
    marker(spawns, "Spawn" .. tag .. "_D", ox + 22, 0, oz + 17)
    marker(spawns, "Spawn" .. tag .. "_Upper", ox + 21, UPPER, oz + 6)
    marker(waypoints, "Waypoint" .. tag .. "_Crate", ox + 6, 1, oz + 18)
end
local n = 0
for bx = 0, BLOCKS - 1 do
    for bz = 0, BLOCKS - 1 do
        n = n + 1
        block(bx * 2 * ROOM, bz * 2 * ROOM, n)
    end
end

Navigation.SetAgentRadius(0.4)
Navigation.SetAgentHeight(1.8)
-- Metres: each limit clears what it links, the 3 m drop and the 1 m crate.
Navigation.SetDropHeight(UPPER + 0.5)
Navigation.SetJumpHeight(1.2)

local match = Scene.CreateEntity("Match")
Scene.AddScript(match, "assets/scripts/match.lua", { kill_limit = p.kills, time_limit = p.time })

Sound.Bake({
    source = { kind = "noise" },
    amp = { a = 0.001, d = 0.08, s = 0.0, r = 0.04 },
}, "C4", "assets/sounds/shot.wav", { duration = 0.15, seed = p.seed })

local puff = Texture.Bake({
    resolution = 64, seed = p.seed, output = "puff",
    nodes = {
        { id = "white", op = "constant", color = { 1, 1, 1, 1 } },
        { id = "disc", op = "shape", kind = "circle", size = { 0.9, 0.9 }, softness = 0.45 },
        { id = "puff", op = "combine_rgb", inputs = { "white", "white", "white", "disc" } },
    },
}, "assets/textures/puff.png", "base_color")

local function emitter(name, r, g, b, size, life, speed)
    local id = Scene.CreateEntity(name)
    Scene.AddComponent(id, "Particles")
    Particles.SetRate(id, 0)
    Particles.SetShape(id, "Sphere", { radius = 0.05 })
    Particles.SetLifetime(id, life / 2, life)
    Particles.SetSpeed(id, speed / 2, speed)
    Particles.SetSize(id, size / 2, size)
    Particles.SetColor(id, r, g, b, 1)
    Component.Set(id, "Particles", { texture = puff })
    return id
end
emitter("Flash", 1.0, 0.85, 0.4, 0.25, 0.06, 0.5)
emitter("Sparks", 1.0, 0.7, 0.3, 0.06, 0.35, 3.0)

local function fighter(name, spawn)
    local id = Scene.CreateEntity(name)
    local x, y, z = Transform.GetPosition(spawn)
    Transform.SetPosition(id, x, y, z)
    Scene.AddComponent(id, "CharacterController")
    CharacterController.SetHeight(id, 1.8)
    CharacterController.SetRadius(id, 0.4)
    CharacterController.SetCenter(id, 0, 0.9, 0)
    Layers.SetLayer(id, FIGHTER)
    return id
end

local function part(parent, kind, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity("Part", kind)
    Scene.SetParent(id, parent)
    Transform.SetLocalPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.Set(id, material)
    return id
end

for i = 1, p.bots do
    local bot = fighter("Bot_" .. i, spawns[(i % #spawns) + 1])
    part(bot, "Sphere", 0, 0.45, 0, 0.9, 0.9, 0.9, "snow")
    part(bot, "Sphere", 0, 1.12, 0, 0.66, 0.66, 0.66, "snow")
    part(bot, "Sphere", 0, 1.58, 0, 0.44, 0.44, 0.44, "snow")
    local nose = part(bot, "Cylinder", 0, 1.58, 0.28, 0.07, 0.09, 0.07, "carrot")
    Transform.SetLocalRotation(nose, 90, 0, 0)
    part(bot, "Box", 0.3, 1.15, 0.3, 0.05, 0.05, 0.6, "stick")
    Scene.AddComponent(bot, "NavMeshAgent")
    NavMeshAgent.SetRadius(bot, 0.4)
    NavMeshAgent.SetSpeed(bot, 4.0)
    NavMeshAgent.SetAcceleration(bot, 12.0)
    NavMeshAgent.SetStoppingDistance(bot, 0.5)
    NavMeshAgent.SetAngularSpeed(bot, 360)
    NavMeshAgent.SetActive(bot, true)
    Scene.AddComponent(bot, "AIPerception")
    AIPerception.SetSightMask(bot, 1 << FIGHTER)
    AIPerception.SetBlockingMask(bot, 1 | (1 << SMOKE))
    AIPerception.SetSightAngle(bot, 75)
    AIPerception.SetSightRange(bot, 40)
    AIPerception.SetLoseSightRange(bot, 45)
    Scene.AddScript(bot, "assets/scripts/snowman.lua")
end

local player = fighter("Player", spawns[1])
Scene.AddScript(player, "assets/scripts/shooter.lua")

local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Light.SetMode(sun, "Mixed") -- live before a bake
Transform.SetRotation(sun, -55, 35, 0)
local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Transform.SetPosition(camera, 3, 1.6, 3)

local SCENE = "assets/scenes/arena.scene"
-- The bakes write their files beside the saved scene.
Scene.Save(SCENE)
Navigation.Bake()
if p.bake then
    Lighting.SetSettings({ texelsPerUnit = 1, samples = 8, bounces = 1,
        denoiser = "None", prioritizeView = false })
    Lighting.Generate()
end
Scene.Save(SCENE)
Application.SetStartupScene(SCENE)
