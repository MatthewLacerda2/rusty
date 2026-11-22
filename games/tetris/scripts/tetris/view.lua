-- Reads the rules' grid coordinates and never decides the game.

local View = {}

local CUBE_SCALE = 0.94   -- of its 1 m cell, leaving a seam
local BURST_TICKS = 90    -- ticks
local PULSE_PEAK = 60.0
local PULSE_DECAY = 0.9   -- share left each tick
local SOUND_AT = { 0, 10, 6 }

View.COLORS = {
    { 0.1, 0.85, 0.9 }, { 0.95, 0.85, 0.1 }, { 0.65, 0.2, 0.85 }, { 0.2, 0.8, 0.25 },
    { 0.9, 0.2, 0.2 }, { 0.2, 0.35, 0.95 }, { 0.95, 0.55, 0.1 }, { 0.5, 0.5, 0.5 },
}

-- The world position of cell (`row`, `col`): the well's floor is y = 0, its centre x = 0.
function View.position(width, row, col)
    return col - (width + 1) / 2, row - 0.5, 0
end

function View.cube(width, kind, row, col)
    local id = Scene.CreateEntity("Cell", "Box")
    Material.Set(id, "block_" .. kind)
    Transform.SetLocalScale(id, CUBE_SCALE, CUBE_SCALE, CUBE_SCALE)
    View.place(width, id, row, col)
    return id
end

function View.place(width, id, row, col)
    Transform.SetPosition(id, View.position(width, row, col))
end

-- Fling a cleared cube out of the well as a rigidbody; `View.tick_bursts` despawns it.
function View.burst(bursts, width, id, col)
    Scene.AddComponent(id, "Collider")
    Scene.AddComponent(id, "RigidBody")
    local side = (col - (width + 1) / 2) / width -- -0.5 at the left wall, 0.5 at the right
    Physics.SetVelocity(id, side * 8, 8 + (col % 3) * 2, 7 + (col % 4))
    Physics.SetAngularVelocity(id, col % 5 - 2, 3, side * 8)
    bursts[#bursts + 1] = { id = id, ticks = BURST_TICKS }
end

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

function View.clear_bursts(bursts)
    for i = #bursts, 1, -1 do
        Scene.DestroyEntity(bursts[i].id)
        bursts[i] = nil
    end
end

function View.pulse(light, intensity, cleared)
    if cleared > 0 then intensity = PULSE_PEAK * (0.5 + cleared / 8) end
    Light.SetIntensity(light, intensity)
    return intensity * PULSE_DECAY
end

function View.sound(name)
    Audio.PlayAt("assets/sounds/" .. name .. ".wav", SOUND_AT[1], SOUND_AT[2], SOUND_AT[3], 0.8)
end

function View.hud()
    local preview = Scene.FindEntityByName("NextGrid")
    return {
        score = Scene.FindEntityByName("ScoreText"),
        level = Scene.FindEntityByName("LevelText"),
        lines = Scene.FindEntityByName("LinesText"),
        over = Scene.FindChild(Scene.FindEntityByName("HUD"), "GameOver"),
        preview = Scene.GetChildren(preview),
    }
end

function View.show_stats(hud, score, level, lines, over)
    Text.SetText(hud.score, "Score  " .. score)
    Text.SetText(hud.level, "Level  " .. level)
    Text.SetText(hud.lines, "Lines  " .. lines)
    Scene.SetActive(hud.over, over)
end

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
