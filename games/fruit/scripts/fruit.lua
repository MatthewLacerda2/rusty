-- A swipe is hit-tested on screen: a body whose projected centre lies within its
-- projected radius of the pointer's path since the last tick is cut.

local View = require("fruit.view")

local Fruit = {
    fields = {
        lives = { type = "number", default = 3, range = { 1, 10 }, tooltip = "Fruit you may drop" },
        bombs = { type = "number", default = 8, range = { 0, 100 },
            tooltip = "One thrown item in this many is a bomb (0: none)" },
        seed = { type = "number", default = 1, tooltip = "Seeds the bursts" },
    },
}

local LAUNCH_Y, LAUNCH_Z = -7, 1.5 -- below the view
local GONE_Y = -9
local FIRST_WAVE, WAVE_GAP = 30, 100 -- ticks
local RESTART_TICKS = 60            -- ticks

local items = {}  -- { id, kind, cut } (cut: a half, never scored)
local hud
local wave_wait, over_for, flash = 0, 0, 0
local last_x, last_y

function Fruit.Throw(_, kind, x, vx, vy)
    local spin = Random.Range(-3.0, 3.0)
    local id = View.throw(kind, x, LAUNCH_Y, LAUNCH_Z, vx, vy, spin)
    items[#items + 1] = { id = id, kind = kind, cut = false }
    return id
end

function Fruit.Halves(_, kind)
    local ids = {}
    for _, item in ipairs(items) do
        if item.cut and item.kind == kind then ids[#ids + 1] = item.id end
    end
    return ids
end

local function burst()
    Fruit.waves = Fruit.waves + 1
    local n = Random.Range(1, 5)
    for _ = 1, n do
        local x = Random.Range(-5.0, 5.0)
        local kind = View.FRUIT[Random.Range(1, #View.FRUIT + 1)]
        if Fruit.bombs > 0 and Fruit.waves > 1 and Random.Range(0, Fruit.bombs) == 0 then
            kind = "bomb"
        end
        Fruit:Throw(kind, x, -x * Random.Range(0.2, 0.5), Random.Range(13.0, 16.0))
    end
end

local function to_segment(px, py, ax, ay, bx, by)
    local dx, dy = bx - ax, by - ay
    local len2 = dx * dx + dy * dy
    local t = 0
    if len2 > 0 then t = math.max(0, math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2)) end
    local cx, cy = ax + t * dx - px, ay + t * dy - py
    return math.sqrt(cx * cx + cy * cy)
end

local function under(item, ax, ay, bx, by)
    local x, y, z = Transform.GetPosition(item.id)
    local sx, sy, depth = Camera.WorldToScreen(x, y, z)
    if depth <= 0 then return false end
    local rx, ry, rz = Camera.GetRight()
    local r = View.radius(item.kind)
    local ex, ey = Camera.WorldToScreen(x + rx * r, y + ry * r, z + rz * r)
    local radius = math.sqrt((ex - sx) ^ 2 + (ey - sy) ^ 2)
    return to_segment(sx, sy, ax, ay, bx, by) <= radius
end

local function game_over()
    Fruit.over, over_for = true, 0
end

local function swipe(ax, ay, bx, by)
    for i = #items, 1, -1 do
        local item = items[i]
        if not item.cut and under(item, ax, ay, bx, by) then
            table.remove(items, i)
            if item.kind == "bomb" then
                flash = View.explode(item.id, hud.blast)
                game_over()
                return
            end
            for _, h in ipairs(View.split(item.id, item.kind, bx - ax, by - ay, hud.juice)) do
                items[#items + 1] = { id = h, kind = item.kind, cut = true }
            end
            Fruit.sliced, Fruit.combo = Fruit.sliced + 1, Fruit.combo + 1
            Fruit.score = Fruit.score + 1
        end
    end
end

local function pointer()
    local _, h = UI.GetScreenSize()
    local mx, my = Input.GetMousePosition()
    local x, y = mx, h - my -- Input is top-left, the screen bottom-left (Camera.md)
    if Input.IsKeyDown("MOUSE0") then
        if last_x ~= nil and not Input.GetKeyDown("MOUSE0") then swipe(last_x, last_y, x, y) end
        last_x, last_y = x, y
        return
    end
    if last_x ~= nil and Fruit.combo >= 3 then Fruit.score = Fruit.score + Fruit.combo end
    Fruit.best_combo = math.max(Fruit.best_combo, Fruit.combo)
    last_x, Fruit.combo = nil, 0
end

local function despawn()
    for i = #items, 1, -1 do
        local item = items[i]
        local _, y = Transform.GetPosition(item.id)
        local _, vy = Physics.GetVelocity(item.id)
        if y < GONE_Y and vy < 0 then
            Scene.DestroyEntity(item.id)
            table.remove(items, i)
            if not item.cut and item.kind ~= "bomb" and not Fruit.over then
                Fruit.missed, Fruit.lives = Fruit.missed + 1, Fruit.lives - 1
                View.sound("miss")
                if Fruit.lives <= 0 then game_over() end
            end
        end
    end
end

function Fruit.Restart()
    for _, item in ipairs(items) do Scene.DestroyEntity(item.id) end
    items = {}
    Random.SetSeed(Fruit.seed)
    Fruit.score, Fruit.sliced, Fruit.missed, Fruit.waves = 0, 0, 0, 0
    Fruit.combo, Fruit.best_combo = 0, 0
    Fruit.lives, Fruit.over = Fruit.max_lives, false
    wave_wait, last_x = FIRST_WAVE, nil
end

function Fruit.Start(id)
    -- Inspector fields arrive as floats; these count whole things.
    for _, f in ipairs({ "lives", "bombs", "seed" }) do Fruit[f] = math.floor(Fruit[f]) end
    Fruit.max_lives = Fruit.lives
    if Fruit.auto == nil then Fruit.auto = true end
    hud = View.find()
    -- Swiped with the cursor: free it and show it (Play starts it locked, hidden).
    Input.SetCursorLocked(false)
    Input.SetCursorVisible(true)
    Fruit.Restart()
end

function Fruit.Update(id, dt)
    despawn()
    if Fruit.over then
        over_for = over_for + 1
        if over_for >= RESTART_TICKS and Input.GetKeyDown("MOUSE0") then Fruit.Restart() end
    else
        pointer()
        if Fruit.auto then
            wave_wait = wave_wait - 1
            if wave_wait <= 0 then burst(); wave_wait = WAVE_GAP end
        end
    end
    flash = View.flash(hud.flash, flash)
    View.show(hud, Fruit.score, Fruit.combo, Fruit.lives, Fruit.over)
end

return Fruit
