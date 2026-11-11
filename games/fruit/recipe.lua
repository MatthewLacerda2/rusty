-- games/fruit/recipe.lua — a slice-the-fruit arcade game in 3D (#2033): fruit
-- (spheres and a capsule with Snyder skins) is thrown up from below a lit kitchen
-- counter in bursts, a mouse swipe cuts it into two halves that fall away with a
-- spray of juice, a bomb ends the game, and fruit left alone costs a life. It
-- proves rigidbodies thrown and despawned in bursts, a swipe hit-tested against
-- moving bodies on screen, splitting an object at runtime, particles, UI text and
-- zimmer sounds.
--
-- Run in edit mode by `make game`: `...` is the command line's `--param` table;
-- the defaults below are what CI runs. `fruit.lua` is the game.

local p = { lives = 3, bombs = 8, seed = 1 }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("fruit: unknown parameter '" .. key .. "'") end
    p[key] = value
end

Scene.New()

-- The skins: one Snyder mottle each fruit's material tints, and the flesh's speckle.
local function ramp(name, scale, lo, hi)
    return Texture.Bake({
        resolution = 64, seed = p.seed,
        nodes = {
            { id = "n", op = "noise", kind = "fbm", scale = scale, octaves = 4 },
            { id = "c", op = "color_ramp", inputs = { "n" },
              stops = { { pos = 0.0, color = lo }, { pos = 1.0, color = hi } } },
        },
    }, "assets/textures/" .. name .. "_base_color.png", "base_color")
end
local skin = ramp("skin", 10.0, { 0.7, 0.7, 0.7, 1 }, { 1, 1, 1, 1 })
local flesh = ramp("flesh", 24.0, { 0.85, 0.85, 0.8, 1 }, { 1, 1, 1, 1 })
local wood = ramp("wood", 6.0, { 0.3, 0.17, 0.07, 1 }, { 0.6, 0.38, 0.18, 1 })
local tile = ramp("tile", 3.0, { 0.55, 0.62, 0.66, 1 }, { 0.85, 0.9, 0.92, 1 })

-- Each kind's skin and flesh colours, in `fruit/view.lua`'s KINDS order.
local kinds = {
    apple = { { 0.8, 0.08, 0.06 }, { 0.98, 0.95, 0.75 } },
    orange = { { 1.0, 0.5, 0.05 }, { 1.0, 0.65, 0.2 } },
    lime = { { 0.35, 0.75, 0.12 }, { 0.75, 0.95, 0.45 } },
    banana = { { 0.98, 0.85, 0.2 }, { 1.0, 0.97, 0.8 } },
    melon = { { 0.12, 0.45, 0.12 }, { 0.95, 0.2, 0.25 } },
}
for name, c in pairs(kinds) do
    Material.DefineAsset(name .. "_skin", { base_color = c[1], base_color_map = skin, roughness = 0.35 })
    Material.DefineAsset(name .. "_flesh", { base_color = c[2], base_color_map = flesh, roughness = 0.6 })
end
Material.DefineAsset("bomb", { base_color = { 0.06, 0.06, 0.07 }, metallic = 0.6, roughness = 0.3 })
Material.DefineAsset("fuse", { base_color = { 1, 0.4, 0.1 }, emissive = { 6, 1.6, 0.3 } })
Material.DefineAsset("wood", { base_color_map = wood, roughness = 0.7 })
Material.DefineAsset("tile", { base_color_map = tile, roughness = 0.5 })

-- The kitchen: a counter across the bottom of the view and a tiled back wall the
-- shadows fall on. Fruit flies in front of both on its own layer (fruit/view.lua),
-- which collides with nothing.
local function solid(name, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity(name, "Box")
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Scene.AddComponent(id, "Collider") -- #2051: a primitive gets none of its own yet
    Material.Set(id, material)
    return id
end
solid("Counter", 0, -4.6, -2, 40, 2, 4, "wood")
solid("Wall", 0, 4, -4.5, 40, 22, 1, "tile")
Layers.SetName(8, "Fruit")
Physics.IgnoreLayerCollision(8, 8)
Physics.IgnoreLayerCollision(8, 0)

local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Transform.SetRotation(sun, -55, 20, 0)
-- The flash a bomb sets off: dark until `fruit.lua` raises it.
local flash = Scene.CreateEntity("Flash", "PointLight")
Transform.SetPosition(flash, 0, 2, 4)
Light.SetColor(flash, 1.0, 0.6, 0.25)
Light.SetRange(flash, 30)
Light.SetIntensity(flash, 0)

local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Transform.SetPosition(camera, 0, 2, 13) -- looking down -Z: +X is screen right, +Y up

-- The juice: a soft disc sprite (#2052: an emitter with no texture draws the
-- checker), one emitter `fruit/view.lua` moves to each cut and tints.
local puff = Texture.Bake({
    resolution = 64, seed = p.seed,
    nodes = {
        { id = "white", op = "constant", color = { 1, 1, 1, 1 } },
        { id = "disc", op = "shape", kind = "circle", size = { 0.9, 0.9 }, softness = 0.45 },
        { id = "puff", op = "combine_rgb", inputs = { "white", "white", "white", "disc" } },
    },
}, "assets/textures/puff.png", "base_color")
local function emitter(name, count, size, life, speed)
    local id = Scene.CreateEntity(name)
    Scene.AddComponent(id, "Particles")
    Particles.SetRate(id, 0)
    Particles.SetShape(id, "Sphere", { radius = 0.2 })
    Particles.SetLifetime(id, life / 2, life)
    Particles.SetSpeed(id, speed / 2, speed)
    Particles.SetSize(id, size / 2, size)
    Component.Set(id, "Particles", { texture = puff, burst_count = count })
    return id
end
emitter("Juice", 40, 0.25, 0.6, 5.0)
emitter("Blast", 120, 0.8, 0.8, 9.0)

-- The sounds: a slice, a bomb, a miss and a short loop under it all.
local function bake(name, patch, note, opts)
    opts.seed = p.seed
    return Sound.Bake(patch, note, "assets/sounds/" .. name .. ".wav", opts)
end
bake("slice", { source = { kind = "noise", color = "white" },
    amp = { a = 0.001, d = 0.09, s = 0.0, r = 0.04 } }, "C6", { duration = 0.1 })
bake("bomb", { source = { kind = "noise", color = "brown" },
    amp = { a = 0.0, d = 0.7, s = 0.1, r = 0.4 } }, "C1", { duration = 0.9 })
bake("miss", { source = { kind = "osc_stack", oscs = { { wave = "square", gain = 0.4 } } },
    amp = { a = 0.005, d = 0.2, s = 0.0, r = 0.1 } }, "E3", { duration = 0.25, glide = { semitones = -5, seconds = 0.2 } })
local loop = Sound.BakeSong({
    bpm = 120, seed = p.seed,
    tracks = { { name = "pluck", gain = 0.4, patch = {
        source = { kind = "osc_stack", oscs = { { wave = "triangle", gain = 1.0 } } },
        amp = { a = 0.003, d = 0.2, s = 0.0, r = 0.1 } } } },
    patterns = { a = { beats = 4, notes = {
        { track = "pluck", note = "C4", start = 0, dur = 0.5 }, { track = "pluck", note = "G4", start = 1, dur = 0.5 },
        { track = "pluck", note = "E4", start = 2, dur = 0.5 }, { track = "pluck", note = "A4", start = 3, dur = 0.5 },
    } } },
    arrangement = { "a" },
}, "assets/sounds/loop.wav")

local game = Scene.CreateEntity("Game")
Scene.AddScript(game, "assets/scripts/fruit.lua", { lives = p.lives, bombs = p.bombs, seed = p.seed })
Scene.AddComponent(game, "AudioSource")
Component.Set(game, "AudioSource", { clip = loop, loop = true, play_on_start = true, volume = 0.3 })

-- The HUD: score and combo top left, lives top right, a banner at game over.
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
Text.SetText(text("ScoreText", 0, 1, 40, -30, 400, 70, 56, 1, 0.95, 0.6), "0")
Text.SetText(text("ComboText", 0, 1, 40, -100, 500, 50, 36, 1, 0.6, 0.2), "")
local lives = text("LivesText", 1, 1, -40, -30, 400, 60, 44, 1, 0.3, 0.25)
Text.SetAlignment(lives, "TopRight")
Text.SetText(lives, string.rep("X ", p.lives))
local over = text("GameOver", 0.5, 0.5, 0, 0, 900, 160, 96, 1, 0.3, 0.25)
Text.SetText(over, "GAME OVER\n<size=36>click to play again</size>")
Text.SetAlignment(over, "MiddleCenter")
Scene.SetActive(over, false)

Scene.Save("assets/scenes/fruit.scene")
Application.SetStartupScene("assets/scenes/fruit.scene")
