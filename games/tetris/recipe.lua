-- games/tetris/recipe.lua — a falling-blocks puzzle in 3D (#2032): play is on a 2D
-- grid, but the well, the pieces and the room are lit cubes seen through a
-- perspective camera, with shadows, a light that pulses on a line clear and the
-- cleared cubes bursting out as rigidbodies. It proves input with repeat timing,
-- UI text and layout, many small meshes changing every few ticks, rigidbodies
-- spawned and despawned in bursts, zimmer sounds and a deterministic game.
--
-- Run in edit mode by `make game`: `...` is the command line's `--param` table;
-- the defaults below are what CI runs. `tetris.lua` is the game.

local p = { width = 10, height = 20, level = 1, seed = 1 }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("tetris: unknown parameter '" .. key .. "'") end
    p[key] = value
end

Scene.New()

-- A Snyder bevelled tile for every block, tinted per piece by its material.
local tile = Texture.Bake({
    resolution = 64, seed = p.seed,
    nodes = {
        { id = "grain", op = "noise", kind = "fbm", scale = 6.0, octaves = 3 },
        { id = "tone", op = "color_ramp", inputs = { "grain" },
          stops = { { pos = 0.0, color = { 0.75, 0.75, 0.75, 1 } },
                    { pos = 1.0, color = { 1.0, 1.0, 1.0, 1 } } } },
    },
}, "assets/textures/block_base_color.png", "base_color")
local colors = {
    { 0.1, 0.85, 0.9 }, { 0.95, 0.85, 0.1 }, { 0.65, 0.2, 0.85 }, { 0.2, 0.8, 0.25 },
    { 0.9, 0.2, 0.2 }, { 0.2, 0.35, 0.95 }, { 0.95, 0.55, 0.1 }, { 0.5, 0.5, 0.5 },
}
for kind, c in ipairs(colors) do
    Material.DefineAsset("block_" .. kind, { base_color = c, base_color_map = tile, roughness = 0.45 })
end
Material.DefineAsset("well", { base_color = { 0.35, 0.38, 0.45 }, roughness = 0.9 })

-- A box with a collider (#2051: a primitive gets none of its own yet).
local function solid(name, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity(name, "Box")
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Scene.AddComponent(id, "Collider")
    Material.Set(id, material)
    return id
end

-- The well: walls, a floor under the grid, a back panel the shadows fall on, and
-- the room's floor the burst cubes land on.
local w, h = p.width, p.height
solid("WallLeft", -w / 2 - 0.5, h / 2 - 0.5, 0, 1, h + 1, 1, "well")
solid("WallRight", w / 2 + 0.5, h / 2 - 0.5, 0, 1, h + 1, 1, "well")
solid("WellFloor", 0, -0.5, 0, w + 2, 1, 1, "well")
solid("Back", 0, h / 2, -0.75, w + 2, h + 2, 0.5, "well")
solid("Ground", 0, -1.5, 6, 60, 1, 30, "well")

local sun = Scene.CreateEntity("Sun", "DirectionalLight")
Transform.SetRotation(sun, -40, 25, 0)
Light.SetCastShadows(sun, true)

-- The light that pulses on a line clear: dark until `tetris.lua` raises it.
local pulse = Scene.CreateEntity("Pulse", "PointLight")
Transform.SetPosition(pulse, 0, h / 2, 4)
Light.SetColor(pulse, 1.0, 0.9, 0.7)
Light.SetRange(pulse, 30)
Light.SetIntensity(pulse, 0)

local camera = Scene.CreateEntity("Camera")
Scene.AddComponent(camera, "Camera")
Transform.SetPosition(camera, -4.5, h / 2 + 1.5, h * 1.6) -- looking down -Z, so +X is screen right

-- The sounds: zimmer one-shots for rotate, lock, clear and game over, and a loop.
local function bake(name, patch, note, opts)
    opts.seed = p.seed
    return Sound.Bake(patch, note, "assets/sounds/" .. name .. ".wav", opts)
end
bake("rotate", { source = { kind = "osc_stack", oscs = { { wave = "square", gain = 0.4 } } },
    amp = { a = 0.002, d = 0.05, s = 0.0, r = 0.03 } }, "E5", { duration = 0.05 })
bake("lock", { source = { kind = "noise", color = "brown" },
    amp = { a = 0.0, d = 0.08, s = 0.0, r = 0.04 } }, "C2", { duration = 0.06 })
bake("clear", { source = { kind = "osc_stack", oscs = { { wave = "saw", gain = 0.5 } } },
    amp = { a = 0.005, d = 0.3, s = 0.2, r = 0.2 } }, "C5", { duration = 0.3, glide = { semitones = -12, seconds = 0.25 } })
bake("over", { source = { kind = "osc_stack", oscs = { { wave = "triangle", gain = 0.6 } } },
    amp = { a = 0.01, d = 0.6, s = 0.3, r = 0.4 } }, "C3", { duration = 0.8, glide = { semitones = 7, seconds = 0.7 } })
local loop = Sound.BakeSong({
    bpm = 132, seed = p.seed,
    tracks = { { name = "bass", gain = 0.5, patch = {
        source = { kind = "osc_stack", oscs = { { wave = "triangle", gain = 1.0 } } },
        amp = { a = 0.005, d = 0.15, s = 0.4, r = 0.1 } } } },
    patterns = { a = { beats = 8, notes = {
        { track = "bass", note = "E2", start = 0, dur = 0.5 }, { track = "bass", note = "E3", start = 1, dur = 0.5 },
        { track = "bass", note = "B2", start = 2, dur = 0.5 }, { track = "bass", note = "G2", start = 3, dur = 0.5 },
        { track = "bass", note = "A2", start = 4, dur = 0.5 }, { track = "bass", note = "A3", start = 5, dur = 0.5 },
        { track = "bass", note = "E2", start = 6, dur = 0.5 }, { track = "bass", note = "B2", start = 7, dur = 0.5 },
    } } },
    arrangement = { "a" },
}, "assets/sounds/loop.wav")

local game = Scene.CreateEntity("Game")
Scene.AddScript(game, "assets/scripts/tetris.lua",
    { width = w, height = h, start_level = p.level, seed = p.seed })
Scene.AddComponent(game, "AudioSource")
Component.Set(game, "AudioSource", { clip = loop, loop = true, play_on_start = true, volume = 0.4 })

-- The HUD: score, level and lines in a column, the next piece in a 4×4 grid, and
-- a game-over banner shown only once the stack tops out.
local canvas = UI.Create("Canvas")
Scene.SetName(canvas, "HUD")
local function element(name, parent, kind, ax, ay, x, y, sw, sh)
    local id = Scene.CreateEntity(name)
    Scene.SetParent(id, parent)
    Scene.AddComponent(id, kind)
    RectTransform.SetAnchorMin(id, ax, ay)
    RectTransform.SetAnchorMax(id, ax, ay)
    RectTransform.SetPivot(id, ax, ay)
    RectTransform.SetAnchoredPosition(id, x, y)
    RectTransform.SetSizeDelta(id, sw, sh)
    return id
end
local side = element("Side", canvas, "Image", 0, 1, 60, -60, 300, 420)
Image.SetColor(side, 0.05, 0.06, 0.09, 0.75)
local stats = element("Stats", side, "LayoutGroup", 0, 1, 0, 0, 300, 180)
LayoutGroup.SetKind(stats, "Vertical")
LayoutGroup.SetPadding(stats, 20, 10, 20, 20)
LayoutGroup.SetSpacing(stats, 0, 8)
for _, name in ipairs({ "Score", "Level", "Lines" }) do
    local text = element(name .. "Text", stats, "Text", 0, 1, 0, 0, 260, 44)
    Text.SetText(text, name)
    Text.SetFontSize(text, 34)
    Text.SetColor(text, 0.92, 0.95, 1.0, 1)
end
local label = element("NextLabel", side, "Text", 0, 1, 20, -190, 260, 40)
Text.SetText(label, "Next")
Text.SetFontSize(label, 28)
Text.SetColor(label, 0.7, 0.75, 0.85, 1)
local grid = element("NextGrid", side, "LayoutGroup", 0, 1, 20, -240, 160, 160)
LayoutGroup.SetKind(grid, "Grid")
LayoutGroup.SetCellSize(grid, 36, 36)
LayoutGroup.SetSpacing(grid, 4, 4)
LayoutGroup.SetConstraint(grid, "FixedColumnCount")
LayoutGroup.SetConstraintCount(grid, 4)
for i = 1, 16 do
    local cell = element("Next" .. i, grid, "Image", 0, 1, 0, 0, 36, 36)
    Image.SetColor(cell, 0, 0, 0, 0.25)
end
local over = element("GameOver", canvas, "Text", 0.5, 0.5, 0, 0, 800, 140)
Text.SetText(over, "GAME OVER\n<size=36>Enter to play again</size>")
Text.SetFontSize(over, 96)
Text.SetAlignment(over, "MiddleCenter")
Text.SetColor(over, 1.0, 0.3, 0.25, 1)
Text.SetOutline(over, 0.1, 0, 0, 0, 1)
Scene.SetActive(over, false)

Scene.Save("assets/scenes/tetris.scene")
Application.SetStartupScene("assets/scenes/tetris.scene")
