-- games/runner/recipe.lua — a plain endless runner (#2034): a capsule runs down
-- three lanes, switching lanes, jumping boxes and sliding under bars, while crates
-- and barrels are dropped onto the track ahead as rigidbodies. The track is built in
-- chunks ahead of the runner and destroyed behind it, and the speed rises. It proves
-- a runner body (kinematic, #2061) at rising speed, a following camera, seeded spawning that
-- runs for as long as the run lasts, rigidbodies dropped at speed, particles and a
-- trail, collisions ending the run and a UI counter.
--
-- Run in edit mode by `make game`: `...` is the command line's `--param` table;
-- the defaults below are what CI runs. `runner.lua` is the game.

local p = { seed = 1, speed = 10, max_speed = 25, accel = 0.15 }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("runner: unknown parameter '" .. key .. "'") end
    p[key] = value
end

Scene.New()

-- Snyder textures: worn planks for the crates, a stone grain for the track.
local function bake(name, seed, dark, light, scale)
    return Texture.Bake({
        resolution = 64, seed = seed,
        nodes = {
            { id = "grain", op = "noise", kind = "fbm", scale = scale, octaves = 4 },
            { id = "tone", op = "color_ramp", inputs = { "grain" },
              stops = { { pos = 0.0, color = dark }, { pos = 1.0, color = light } } },
        },
    }, "assets/textures/" .. name .. "_base_color.png", "base_color")
end
local wood = bake("crate", p.seed, { 0.3, 0.16, 0.06, 1 }, { 0.72, 0.5, 0.26, 1 }, 9.0)
local stone = bake("track", p.seed + 1, { 0.32, 0.33, 0.36, 1 }, { 0.55, 0.56, 0.6, 1 }, 5.0)
Material.DefineAsset("track", { base_color_map = stone, roughness = 0.9 })
Material.DefineAsset("rail", { base_color = { 0.2, 0.22, 0.28 }, roughness = 0.7 })
Material.DefineAsset("crate", { base_color_map = wood, roughness = 0.8 })
Material.DefineAsset("barrel", { base_color = { 0.75, 0.2, 0.12 }, roughness = 0.5, metallic = 0.6 })
Material.DefineAsset("box", { base_color = { 0.95, 0.6, 0.1 }, roughness = 0.6 })
Material.DefineAsset("bar", { base_color = { 0.9, 0.15, 0.2 }, roughness = 0.5 })
Material.DefineAsset("wall", { base_color = { 0.25, 0.45, 0.85 }, roughness = 0.6 })
Material.DefineAsset("runner", { base_color = { 0.95, 0.95, 1.0 }, roughness = 0.35 })

-- The start pad: the first 40 m of track, clear of obstacles, and 20 m behind the
-- start. `runner/track.lua` builds the rest as the runner goes.
local function slab(name, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity(name, "Box")
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Scene.AddComponent(id, "Collider") -- #2051: a primitive gets none of its own yet
    Scene.SetStatic(id, true)
    Material.Set(id, material)
end
slab("StartFloor", 0, -0.5, 10, 9.0, 1, 60, "track")
slab("StartRailLeft", 4.3, 0.3, 10, 0.4, 0.6, 60, "rail")
slab("StartRailRight", -4.3, 0.3, 10, 0.4, 0.6, 60, "rail")

-- The runner: a kinematic body, its feet at its origin, that `runner.lua` moves
-- itself (#2061: a CharacterController stalls on a flat floor in Play). The drawn
-- capsule child carries its collider, on layer 8 so the runner's own ground
-- probe and crash test skip it, and the trail; `runner.lua` squashes it for a slide.
local runner = Scene.CreateEntity("Runner")
Scene.AddComponent(runner, "RigidBody")
Physics.SetKinematic(runner, true)
local body = Scene.CreateEntity("Body", "Capsule")
Scene.SetParent(body, runner)
Transform.SetLocalPosition(body, 0, 0.9, 0)
Transform.SetLocalScale(body, 0.8, 0.9, 0.8)
Scene.AddComponent(body, "Collider")
Physics.SetColliderShape(body, { kind = "Capsule", radius = 0.5, height = 2 })
Layers.SetLayer(body, 8)
Material.Set(body, "runner")
Scene.AddComponent(body, "Trail")
Trail.SetTime(body, 0.35)
Trail.SetWidth(body, 0.5, 0)
Trail.SetColors(body, 0.4, 0.8, 1.6, 0.8, 0.2, 0.4, 1.0, 0)
Trail.SetBlend(body, "Additive")

local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Transform.SetRotation(sun, -50, 35, 0)
Light.SetCastShadows(sun, true)

local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Transform.SetPosition(camera, 0, 3.2, -6.5)
Transform.SetRotation(camera, 15, 0, 0)

-- The particles' sprite: a soft white disc (#2052: no texture draws the checker).
local puff = Texture.Bake({
    resolution = 64, seed = p.seed, output = "puff",
    nodes = {
        { id = "white", op = "constant", color = { 1, 1, 1, 1 } },
        { id = "disc", op = "shape", kind = "circle", size = { 0.9, 0.9 }, softness = 0.45 },
        { id = "puff", op = "combine_rgb", inputs = { "white", "white", "white", "disc" } },
    },
}, "assets/textures/puff.png", "base_color")
local function emitter(name, r, g, b, size, life, speed, mode)
    local id = Scene.CreateEntity(name)
    Scene.AddComponent(id, "Particles")
    Particles.SetRate(id, 0)
    Particles.SetShape(id, "Hemisphere", { radius = 0.3 })
    Particles.SetDirection(id, 0, 1, 0)
    Particles.SetLifetime(id, life / 2, life)
    Particles.SetSpeed(id, speed / 2, speed)
    Particles.SetSize(id, size / 2, size)
    Particles.SetColor(id, r, g, b, 1)
    if mode then Particles.SetRenderMode(id, mode) end
    Component.Set(id, "Particles", { texture = puff, burst_count = 16 })
    return id
end
emitter("Dust", 0.75, 0.68, 0.55, 0.6, 0.8, 1.5)
local sparks = emitter("Sparks", 1.6, 1.0, 0.4, 0.08, 0.3, 6.0, "stretched")
Particles.SetStretch(sparks, 1, 0.04)

-- The sounds `runner.lua` plays: zimmer one-shots for a jump, a slide, a crate
-- landing, a near miss and the crash.
local function sound(name, patch, note, opts)
    opts.seed = p.seed
    return Sound.Bake(patch, note, "assets/sounds/" .. name .. ".wav", opts)
end
sound("jump", { source = { kind = "osc_stack", oscs = { { wave = "square", gain = 0.4 } } },
    amp = { a = 0.002, d = 0.12, s = 0.0, r = 0.05 } }, "C5", { duration = 0.12, glide = { semitones = 7, seconds = 0.1 } })
sound("slide", { source = { kind = "noise", color = "pink" },
    amp = { a = 0.01, d = 0.25, s = 0.0, r = 0.1 } }, "C3", { duration = 0.3 })
sound("thud", { source = { kind = "noise", color = "brown" },
    amp = { a = 0.0, d = 0.1, s = 0.0, r = 0.05 } }, "C2", { duration = 0.1 })
sound("whoosh", { source = { kind = "noise", color = "white" },
    amp = { a = 0.02, d = 0.15, s = 0.0, r = 0.05 } }, "C4", { duration = 0.18 })
sound("crash", { source = { kind = "osc_stack", oscs = { { wave = "saw", gain = 0.6 } } },
    amp = { a = 0.005, d = 0.5, s = 0.2, r = 0.3 } }, "C3", { duration = 0.6, glide = { semitones = -12, seconds = 0.5 } })

local game = Scene.CreateEntity("Game")
Scene.AddScript(game, "assets/scripts/runner.lua",
    { seed = p.seed, start_speed = p.speed, max_speed = p.max_speed, accel = p.accel })

-- The HUD: the distance counter, and a banner shown once the run ends.
local canvas = UI.Create("Canvas")
Scene.SetName(canvas, "HUD")
local function text(name, ax, ay, x, y, w, h, size, r, g, b)
    local id = Scene.CreateEntity(name)
    Scene.SetParent(id, canvas)
    Scene.AddComponent(id, "Text")
    RectTransform.SetAnchorMin(id, ax, ay)
    RectTransform.SetAnchorMax(id, ax, ay)
    RectTransform.SetPivot(id, ax, ay)
    RectTransform.SetAnchoredPosition(id, x, y)
    RectTransform.SetSizeDelta(id, w, h)
    Text.SetFontSize(id, size)
    Text.SetColor(id, r, g, b, 1)
    Text.SetOutline(id, 0.08, 0, 0, 0, 1)
    return id
end
local distance = text("Distance", 0, 1, 40, -30, 500, 70, 56, 1, 1, 1)
Text.SetText(distance, "0 m")
local over = text("GameOver", 0.5, 0.5, 0, 0, 900, 160, 96, 1.0, 0.3, 0.25)
Text.SetText(over, "CRASHED\n<size=36>Enter to run again</size>")
Text.SetAlignment(over, "MiddleCenter")
Scene.SetActive(over, false)

Scene.Save("assets/scenes/runner.scene")
Application.SetStartupScene("assets/scenes/runner.scene")
