-- level.lua — one level's rules and HUD. Its targets are the bodies under
-- `Bodies` whose names start "Target"; the level is cleared when every one has
-- fallen below the platforms' top. Then the next level loads in the background
-- (Scene.LoadAsync) behind the `Loading` canvas, while this one keeps running.

local Level = {
    fields = {
        number = { type = "number", default = 1 },
        top = { type = "number", default = 1.0, tooltip = "Height of the platforms' top, metres" },
        next = { type = "string", default = "", tooltip = "The scene loaded once cleared" },
    },
}

local FALLEN = 0.25 -- metres below the top: a target that low is off its platform
local WAIT = 2.5    -- seconds between clearing and the next level

local function collect(id, into)
    for _, child in ipairs(Scene.GetChildren(id)) do
        local name = Scene.GetName(child)
        if name:sub(1, 6) == "Target" then into[#into + 1] = child end
        collect(child, into)
    end
    return into
end

function Level.Start(id)
    Level.targets = collect(Scene.FindEntityByName("Bodies"), {})
    Level.cleared = false
    Level.wait = WAIT
    Level.status = Scene.FindEntityByName("Status")
    Level.banner = Scene.FindEntityByName("Banner")
    Level.loading = Scene.FindEntityByName("Loading")
    Level.loading_text = Scene.FindEntityByName("LoadingText")
    Scene.SetActive(Level.loading, false) -- found while active, then hidden
    Level.op = nil
end

-- Start loading the next level behind the loading canvas; its operation.
function Level.LoadNext()
    if not Level.op then
        Scene.SetActive(Level.loading, true)
        Level.op = Scene.LoadAsync(Level.next)
    end
    return Level.op
end

-- The loading canvas follows the operation; a load that failed hides it again.
local function show_loading()
    if Scene.GetLoadStatus(Level.op) == "failed" then
        Scene.SetActive(Level.loading, false)
        Text.SetText(Level.banner, "The next level could not be loaded")
        return
    end
    local percent = math.floor(Scene.GetLoadProgress(Level.op) * 100)
    Text.SetText(Level.loading_text, string.format("Loading level %d   %d%%", Level.number + 1, percent))
end

-- How many targets still stand on a platform.
function Level.Standing()
    local n = 0
    for _, target in ipairs(Level.targets) do
        local _, y = Transform.GetPosition(target)
        if y > Level.top - FALLEN then n = n + 1 end
    end
    return n
end

function Level.Update(id, dt)
    local standing = Level.Standing()
    local thrower = Scene.GetScript(Scene.FindEntityByName("Player"), "thrower")
    local balls = thrower and thrower.left or 0
    Text.SetText(Level.status, string.format("Level %d    targets %d    balls %d",
        Level.number, standing, balls))
    if standing == 0 and not Level.cleared then
        Level.cleared = true
        Text.SetText(Level.banner, Level.next ~= "" and "Cleared!" or "Every level cleared!")
    elseif not Level.cleared and balls == 0 then
        Text.SetText(Level.banner, "Out of balls: R to retry")
    end
    if Level.op then
        show_loading()
    elseif Level.cleared and Level.next ~= "" then
        Level.wait = Level.wait - dt
        if Level.wait <= 0 then Level.LoadNext() end
    end
end

return Level
