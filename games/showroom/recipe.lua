-- A lit showroom: sun through an east window, so the east half is bright and the
-- west dark. `...` is the command line's `--param` table.

local p = { seed = 1, bake = false }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("showroom: unknown parameter '" .. key .. "'") end
    p[key] = value
end

local SCENE = "assets/scenes/showroom.scene"
local W, D, H = 6, 4, 3.5 -- half width (X), half depth (Z), height, m
local BALL_Z, BALL_TO = -1.0, 3.5

Scene.New()
Lighting.SetAmbientColor(0.02, 0.02, 0.025)

local function block(name, material, x, y, z, sx, sy, sz)
    local id = Scene.CreateEntity(name, "Box")
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.Set(id, material)
    Scene.SetStatic(id, true)
    return id
end

-- The floor's albedo is projected straight down so the marble's veins keep their size in metres.
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

block("Floor", "floor", 0, -0.1, 0, 2 * W + 0.4, 0.2, 2 * D + 0.4)
block("Ceiling", "wall", 0, H + 0.1, 0, 2 * W + 0.4, 0.2, 2 * D + 0.4)
block("WallWest", "wall", -W - 0.1, H / 2, 0, 0.2, H, 2 * D)
block("WallNorth", "wall", 0, H / 2, D + 0.1, 2 * W + 0.4, H, 0.2)
block("WallSouth", "wall", 0, H / 2, -D - 0.1, 2 * W + 0.4, H, 0.2)
local sill, lintel, half = 1.0, 2.8, 1.5 -- m
block("WallEastLow", "wall", W + 0.1, sill / 2, 0, 0.2, sill, 2 * D)
block("WallEastHigh", "wall", W + 0.1, (lintel + H) / 2, 0, 0.2, H - lintel, 2 * D)
block("WallEastSouth", "wall", W + 0.1, (sill + lintel) / 2, -(D + half) / 2,
    0.2, lintel - sill, D - half)
block("WallEastNorth", "wall", W + 0.1, (sill + lintel) / 2, (D + half) / 2,
    0.2, lintel - sill, D - half)
local pane = block("WindowPane", "window_glass", W + 0.1, (sill + lintel) / 2, 0,
    0.02, lintel - sill, 2 * half)

local tinted = block("TintedPane", "tinted_glass", W - 1.5, 1.2, 0.5, 0.03, 2.0, 1.6)

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

local pedestal = Scene.CreateEntity("Pedestal", "Sphere")
Transform.SetPosition(pedestal, 2.5, 0.5, 2.5)
Material.Set(pedestal, "lacquer")
Scene.SetStatic(pedestal, true)

local sign = block("Sign", "sign", -1.5, 2.4, D - 0.05, 2.4, 0.4, 0.06)
local hum = Sound.Bake({
    source = { kind = "osc_stack", oscs = { { wave = "saw", gain = 0.3 },
        { wave = "sine", gain = 1.0, octave = 1 } } },
    amp = { a = 0.05, d = 0.0, s = 1.0, r = 0.05 },
}, "A1", "assets/sounds/hum.wav", { duration = 1.0, seed = p.seed })
Scene.AddComponent(sign, "AudioSource")
Component.Set(sign, "AudioSource", { clip = hum, loop = true, play_on_start = true,
    volume = 0.2 })

local ball = Scene.CreateEntity("Ball", "Sphere")
Transform.SetPosition(ball, -W + 1.0, 0.3, BALL_Z)
Transform.SetLocalScale(ball, 0.6, 0.6, 0.6)
Material.Set(ball, "ball")
Scene.AddComponent(ball, "LightProbeProxyVolume")
Scene.AddScript(ball, "assets/scripts/roll.lua", { speed = 1.5, radius = 0.3,
    to_x = BALL_TO })

local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Light.SetMode(sun, "Mixed")
Transform.SetRotation(sun, -35, 90, 0) -- a light shines along its -Z
Light.SetIntensity(sun, 3.0)

local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Scene.AddComponent(camera, "PostProcessing")
Transform.SetPosition(camera, -4.5, 1.6, -3.2)
Transform.SetRotation(camera, 0, 60, 0)
Graphics.SetSsrActive(true)
Graphics.SetSsrMinSmoothness(0.1)

-- Few and small probes: each costs a GPU capture per bounce, and the generation
-- has to take seconds on a software adapter.
for _, x in ipairs({ -W + 1, 0, BALL_TO }) do
    for _, y in ipairs({ 0.4, 2.0 }) do Probe.Add(x, y, BALL_Z) end
end
local box = Reflection.Add(-W / 2, 1.6, 0, W / 2, H / 2, D)
Reflection.Set(box, { rotation = { x = 0, y = 15, z = 0 } })
local sphere = Reflection.Add(W / 2, 1.6, 0, W / 2, H / 2, D)
Reflection.Set(sphere, { shape = "Sphere", radius = 3.5 })
Lighting.SetSettings({ reflectionResolution = 64 })

if p.bake then
    Scene.CreateEntity("BakeRequested")
end
Scene.Save(SCENE)
if p.bake then
    Lighting.SetSettings({ texelsPerUnit = 1, samples = 8, bounces = 1,
        maxLightmapSize = 64, denoiser = "None", prioritizeView = false })
    Lighting.Generate()
else
    Probe.Bake()
    Reflection.Bake()
end
Scene.Save(SCENE)
Application.SetStartupScene(SCENE)
