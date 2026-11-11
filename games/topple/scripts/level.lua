-- level.lua — one level's rules and HUD. Its targets are the bodies under
-- `Bodies` whose names start "Target"; the level is cleared when every one has
-- fallen below the platforms' top. Then the next level loads.

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
    if Level.cleared and Level.next ~= "" then
        Level.wait = Level.wait - dt
        if Level.wait <= 0 then Scene.Load(Level.next) end
    end
end

return Level
