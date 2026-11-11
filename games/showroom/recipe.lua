-- games/showroom/recipe.lua — a lit showroom (#2031): one interior room built to
-- exercise lighting and reflections, not gameplay.
--
-- A 12 × 8 m room with a window in its east wall, the sun coming through it, so
-- the east half is bright and the west half dark. In it: a polished floor (SSR at
-- a low minimum smoothness), a mirror on the west wall, a clear window pane, a
-- tinted freestanding pane in front of it, a drinking glass on a table in front of
-- the mirror, an emissive sign humming on the north wall, a turned box and a
-- sphere reflection probe, a grid of light probes, and a ball (`roll.lua`) rolling
-- from the dark half into the bright one, lit by the probes through a Light Probe
-- Proxy Volume. `bake = true` adds a tiny lightmap bake (`Lighting.Generate`).
-- Run in edit mode by `make game`: `...` is the command line's `--param` table;
-- the defaults below are what CI runs.

local p = { seed = 1, bake = false }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("showroom: unknown parameter '" .. key .. "'") end
    p[key] = value
end

local SCENE = "assets/scenes/showroom.scene"
local W, D, H = 6, 4, 3.5 -- the room's half width (X), half depth (Z), height
-- The ball's line, inside the strip the window lights, and where it stops: in the
-- sun patch on the floor.
local BALL_Z, BALL_TO = -1.0, 3.5

Scene.New()
Lighting.SetAmbientColor(0.02, 0.02, 0.025)

-- A static box from its centre and full size, wearing `material`.
local function block(name, material, x, y, z, sx, sy, sz)
    local id = Scene.CreateEntity(name, "Box")
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.Set(id, material)
    Scene.SetStatic(id, true)
    return id
end

-- Materials. The floor's albedo is a Snyder marble, projected straight down so
-- its veins keep their size in metres; it is smooth enough for SSR.
local marble = Texture.Bake({
    resolution = 128, seed = p.seed,
    nodes = {
        { id = "veins", op = "noise", kind = "fbm", scale = 6.0, octaves = 5 },
        { id = "stone", op = "color_ramp", inputs = { "veins" },
          stops = { { pos = 0.0, color = { 0.55, 0.55, 0.58, 1 } },
                    { pos = 0.5, color = { 0.85, 0.84, 0.82, 1 } },
                    { pos = 1.0, color = { 0.35, 0.36, 0.4, 1 } } } },
    },
}, "assets/textures/floor_base_color.png", "base_color")
Material.DefineAsset("floor", { base_color_map = marble, roughness = 0.12,
    uv_mapping = "Planar", world_size = 2.0 })
Material.DefineAsset("wall", { base_color = { 0.7, 0.68, 0.64 }, roughness = 0.9 })
Material.DefineAsset("mirror", { base_color = { 0.95, 0.95, 0.95 }, metallic = 1.0,
    roughness = 0.0 })
Material.DefineAsset("wood", { base_color = { 0.35, 0.2, 0.1 }, roughness = 0.6 })
-- A lacquered pedestal: a dielectric's F0 from `ior` and `KHR_materials_specular`.
Material.DefineAsset("lacquer", { base_color = { 0.1, 0.25, 0.6 }, roughness = 0.25,
    ior = 1.8, specular = 1.0, specular_color = { 1.0, 0.85, 0.6 } })
Material.DefineAsset("window_glass", { render_mode = "Transparent", alpha = 0.08,
    base_color = { 0.9, 0.95, 1.0 }, roughness = 0.02, ior = 1.5 })
Material.DefineAsset("tinted_glass", { render_mode = "Transparent", alpha = 0.25,
    base_color = { 0.3, 0.8, 0.5 }, roughness = 0.05, ior = 1.5,
    transmittance_color = { 0.3, 0.8, 0.5 }, absorption_distance = 0.5 })
Material.DefineAsset("drinking_glass", { render_mode = "Transparent", alpha = 0.15,
    base_color = { 1, 1, 1 }, roughness = 0.0, ior = 1.5, render_face = "Both" })
Material.DefineAsset("sign", { base_color = { 0.1, 0.02, 0.01 }, roughness = 0.5,
    emissive = { 6.0, 1.2, 0.3 } })
Material.DefineAsset("ball", { base_color = { 0.85, 0.85, 0.85 }, roughness = 0.4 })

-- The shell: floor, ceiling, three plain walls, and the east wall built round a
-- window 3 m wide and 1.8 m tall.
block("Floor", "floor", 0, -0.1, 0, 2 * W + 0.4, 0.2, 2 * D + 0.4)
block("Ceiling", "wall", 0, H + 0.1, 0, 2 * W + 0.4, 0.2, 2 * D + 0.4)
block("WallWest", "wall", -W - 0.1, H / 2, 0, 0.2, H, 2 * D)
block("WallNorth", "wall", 0, H / 2, D + 0.1, 2 * W + 0.4, H, 0.2)
block("WallSouth", "wall", 0, H / 2, -D - 0.1, 2 * W + 0.4, H, 0.2)
local sill, lintel, half = 1.0, 2.8, 1.5 -- the window's bottom, top and half width
block("WallEastLow", "wall", W + 0.1, sill / 2, 0, 0.2, sill, 2 * D)
block("WallEastHigh", "wall", W + 0.1, (lintel + H) / 2, 0, 0.2, H - lintel, 2 * D)
block("WallEastSouth", "wall", W + 0.1, (sill + lintel) / 2, -(D + half) / 2,
    0.2, lintel - sill, D - half)
block("WallEastNorth", "wall", W + 0.1, (sill + lintel) / 2, (D + half) / 2,
    0.2, lintel - sill, D - half)
local pane = block("WindowPane", "window_glass", W + 0.1, (sill + lintel) / 2, 0,
    0.02, lintel - sill, 2 * half)

-- Glass in front of glass: a tinted pane standing between the room and the window.
local tinted = block("TintedPane", "tinted_glass", W - 1.5, 1.2, 0.5, 0.03, 2.0, 1.6)

-- The west wall's mirror, and a table in front of it holding a drinking glass, so
-- the glass is seen against the mirror.
block("Mirror", "mirror", -W + 0.03, 1.5, 0, 0.04, 1.8, 2.6)
block("Table", "wood", -W + 1.2, 0.4, 0, 0.8, 0.8, 1.2)
local glass = Scene.CreateEntity("Glass", "Cylinder")
Transform.SetPosition(glass, -W + 1.2, 0.92, 0)
Transform.SetLocalScale(glass, 0.08, 0.12, 0.08)
Material.Set(glass, "drinking_glass")
Scene.SetStatic(glass, true)
-- Glass lets the sun through: a Mesh casts a solid shadow unless told not to, as
-- in Unity.
for _, id in ipairs({ pane, tinted, glass }) do
    Component.Set(id, "Mesh", { cast_shadows = "Off" })
end

-- A pedestal in the bright half, for the specular tint.
local pedestal = Scene.CreateEntity("Pedestal", "Sphere")
Transform.SetPosition(pedestal, 2.5, 0.5, 2.5)
Material.Set(pedestal, "lacquer")
Scene.SetStatic(pedestal, true)

-- The emissive sign on the north wall, with the hum of its tube.
local sign = block("Sign", "sign", -1.5, 2.4, D - 0.05, 2.4, 0.4, 0.06)
local hum = Sound.Bake({
    source = { kind = "osc_stack", oscs = { { wave = "saw", gain = 0.3 },
        { wave = "sine", gain = 1.0, octave = 1 } } },
    amp = { a = 0.05, d = 0.0, s = 1.0, r = 0.05 },
}, "A1", "assets/sounds/hum.wav", { duration = 1.0, seed = p.seed })
Scene.AddComponent(sign, "AudioSource")
Component.Set(sign, "AudioSource", { clip = hum, loop = true, play_on_start = true,
    volume = 0.2 })

-- The ball, rolling from the dark west half into the sunlit east one, lit by the
-- probes per pixel through its proxy volume.
local ball = Scene.CreateEntity("Ball", "Sphere")
Transform.SetPosition(ball, -W + 1.0, 0.3, BALL_Z)
Transform.SetLocalScale(ball, 0.6, 0.6, 0.6)
Material.Set(ball, "ball")
Scene.AddComponent(ball, "LightProbeProxyVolume")
Scene.AddScript(ball, "assets/scripts/roll.lua", { speed = 1.5, radius = 0.3,
    to_x = BALL_TO })

-- Daylight through the window, falling westward onto the floor.
local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Transform.SetRotation(sun, -35, 90, 0) -- a light shines along its -Z
Light.SetIntensity(sun, 3.0)

-- The camera, with SSR reaching down to rough-ish surfaces.
local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Scene.AddComponent(camera, "PostProcessing")
Transform.SetPosition(camera, -4.5, 1.6, -3.2)
Transform.SetRotation(camera, 0, 60, 0)
Graphics.SetSsrActive(true)
Graphics.SetSsrMinSmoothness(0.1)

-- Probes: light probes along the ball's path (from the dark half, under the
-- window, to the sunlit patch), a box probe turned 15° over the west half and a
-- sphere over the east half. Few and small, since each costs a GPU capture per
-- bounce and the generation has to take seconds on a software adapter.
for _, x in ipairs({ -W + 1, 0, BALL_TO }) do
    for _, y in ipairs({ 0.4, 2.0 }) do Probe.Add(x, y, BALL_Z) end
end
local box = Reflection.Add(-W / 2, 1.6, 0, W / 2, H / 2, D)
Reflection.Set(box, { rotation = { x = 0, y = 15, z = 0 } })
local sphere = Reflection.Add(W / 2, 1.6, 0, W / 2, H / 2, D)
Reflection.Set(sphere, { shape = "Sphere", radius = 3.5 })
Lighting.SetSettings({ reflectionResolution = 64 })

if p.bake then
    -- A marker the bake scenario reads: this generation asked for a bake.
    Scene.CreateEntity("BakeRequested")
end
Scene.Save(SCENE)
if p.bake then
    -- The lowest resolution the bake offers: it takes seconds.
    Lighting.SetSettings({ texelsPerUnit = 1, samples = 8, bounces = 1,
        maxLightmapSize = 64, denoiser = "None", prioritizeView = false })
    Lighting.Generate()
else
    Probe.Bake()
    Reflection.Bake()
end
Scene.Save(SCENE)
Application.SetStartupScene(SCENE)
