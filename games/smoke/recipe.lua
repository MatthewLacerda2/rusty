-- `...` is the command line's `--param` table; the defaults are what CI runs.

local p = { size = 20, seed = 1 }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("smoke: unknown parameter '" .. key .. "'") end
    p[key] = value
end

Scene.New()

local floor = Scene.CreateEntity("Floor", "Plane")
Transform.SetLocalScale(floor, p.size / 15, 1, p.size / 15)

local albedo = Texture.Bake({
    resolution = 64, seed = p.seed,
    nodes = {
        { id = "grain", op = "noise", kind = "fbm", scale = 8.0, octaves = 4 },
        { id = "wood", op = "color_ramp", inputs = { "grain" },
          stops = { { pos = 0.0, color = { 0.25, 0.12, 0.04, 1 } },
                    { pos = 1.0, color = { 0.75, 0.5, 0.25, 1 } } } },
    },
}, "assets/textures/crate_base_color.png", "base_color")
Material.DefineAsset("crate", { base_color_map = albedo, roughness = 0.8 })

local crate = Scene.CreateEntity("Crate", "Box")
Transform.SetPosition(crate, 0, 0.5, 0)
Material.Set(crate, "crate")
Scene.AddScript(crate, "assets/scripts/mover.lua", { speed = 1.0 })

local blip = Sound.Bake({
    source = { kind = "osc_stack", oscs = { { wave = "sine", gain = 1.0 } } },
    amp = { a = 0.005, d = 0.1, s = 0.0, r = 0.05 },
}, "A4", "assets/sounds/blip.wav", { duration = 0.2, seed = p.seed })
Scene.AddComponent(crate, "AudioSource")
Component.Set(crate, "AudioSource", { clip = blip, loop = true, play_on_start = true })

local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Light.SetMode(sun, "Mixed")
Transform.SetRotation(sun, -50, 30, 0)

local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Transform.SetPosition(camera, 0, 4, -8)

Scene.Save("assets/scenes/smoke.scene")
Application.SetStartupScene("assets/scenes/smoke.scene")
