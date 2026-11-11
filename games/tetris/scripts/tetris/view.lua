-- tetris/view.lua — what the player sees and hears of the board: one lit cube per
-- filled cell, the line-clear burst (the cleared cubes become rigidbodies flung
-- out of the well, then despawn), the light that pulses on a clear, the sounds and
-- the HUD. It reads the rules' grid coordinates and never decides the game.

local View = {}

local CUBE_SCALE = 0.94   -- a cube's size in its 1 m cell, leaving a seam between cells
local BURST_TICKS = 90    -- how long a burst cube flies before it despawns
local PULSE_PEAK = 60.0   -- the pulse light's intensity the tick a line clears
local PULSE_DECAY = 0.9   -- how much of the pulse is left each tick after that
local SOUND_AT = { 0, 10, 6 } -- where the one-shot sounds play, before the well

-- The colour of each kind (and of a staged garbage cell), for the HUD's preview.
View.COLORS = {
    { 0.1, 0.85, 0.9 }, { 0.95, 0.85, 0.1 }, { 0.65, 0.2, 0.85 }, { 0.2, 0.8, 0.25 },
    { 0.9, 0.2, 0.2 }, { 0.2, 0.35, 0.95 }, { 0.95, 0.55, 0.1 }, { 0.5, 0.5, 0.5 },
}

-- The world position of cell (`row`, `col`) on a board `width` wide: the well's
-- floor is y = 0 and its centre x = 0.
function View.position(width, row, col)
    return col - (width + 1) / 2, row - 0.5, 0
end

-- A new cube of `kind` at cell (`row`, `col`).
function View.cube(width, kind, row, col)
    local id = Scene.CreateEntity("Cell", "Box")
    Material.Set(id, "block_" .. kind)
    Transform.SetLocalScale(id, CUBE_SCALE, CUBE_SCALE, CUBE_SCALE)
    View.place(width, id, row, col)
    return id
end

-- Move cube `id` to cell (`row`, `col`).
function View.place(width, id, row, col)
    Transform.SetPosition(id, View.position(width, row, col))
end

-- Fling cube `id`, cleared from column `col`, out of the well as a rigidbody, and
-- remember it in `bursts` so `View.tick_bursts` despawns it.
function View.burst(bursts, width, id, col)
    Scene.AddComponent(id, "Collider")
    Scene.AddComponent(id, "RigidBody")
    local side = (col - (width + 1) / 2) / width -- -0.5 at the left wall, 0.5 at the right
    Physics.SetVelocity(id, side * 8, 8 + (col % 3) * 2, 7 + (col % 4))
    Physics.SetAngularVelocity(id, col % 5 - 2, 3, side * 8)
    bursts[#bursts + 1] = { id = id, ticks = BURST_TICKS }
end

-- Count every burst cube down a tick, destroying the ones whose time is up.
function View.tick_bursts(bursts)
    for i = #bursts, 1, -1 do
        local b = bursts[i]
        b.ticks = b.ticks - 1
        if b.ticks <= 0 then
            Scene.DestroyEntity(b.id)
            table.remove(bursts, i)
        end
    end
end

-- Destroy every burst cube at once (a restart).
function View.clear_bursts(bursts)
    for i = #bursts, 1, -1 do
        Scene.DestroyEntity(bursts[i].id)
        bursts[i] = nil
    end
end

-- The pulse light's intensity for the next tick, from this one's.
function View.pulse(light, intensity, cleared)
    if cleared > 0 then intensity = PULSE_PEAK * (0.5 + cleared / 8) end
    Light.SetIntensity(light, intensity)
    return intensity * PULSE_DECAY
end

-- Play one of the game's one-shot sounds (`rotate`, `lock`, `clear`, `over`).
function View.sound(name)
    Audio.PlayAt("assets/sounds/" .. name .. ".wav", SOUND_AT[1], SOUND_AT[2], SOUND_AT[3], 0.8)
end

-- The HUD's entities, found by name once Play starts.
function View.hud()
    local preview = Scene.FindEntityByName("NextGrid")
    return {
        score = Scene.FindEntityByName("ScoreText"),
        level = Scene.FindEntityByName("LevelText"),
        lines = Scene.FindEntityByName("LinesText"),
        over = Scene.FindChild(Scene.FindEntityByName("HUD"), "GameOver"), -- inactive until the end
        preview = Scene.GetChildren(preview),
    }
end

-- Write the score, level, lines and game-over state into the HUD.
function View.show_stats(hud, score, level, lines, over)
    Text.SetText(hud.score, "Score  " .. score)
    Text.SetText(hud.level, "Level  " .. level)
    Text.SetText(hud.lines, "Lines  " .. lines)
    Scene.SetActive(hud.over, over)
end

-- Draw the next piece into the preview's 4×4 grid of images (row 1 at the top).
function View.show_next(hud, cells, kind)
    local lit = {}
    for _, c in ipairs(cells) do lit[(3 - c[2]) * 4 + c[1] + 1] = true end
    local color = View.COLORS[kind]
    for i, image in ipairs(hud.preview) do
        if lit[i] then
            Image.SetColor(image, color[1], color[2], color[3], 1)
        else
            Image.SetColor(image, 0, 0, 0, 0.25)
        end
    end
end

return View
