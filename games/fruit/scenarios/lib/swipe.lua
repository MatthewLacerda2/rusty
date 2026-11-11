-- scenarios/lib/swipe.lua — the helpers the fruit scenarios share, loaded with
-- `dofile` (a scenario is only a top-level `scenarios/*.lua`, so this is none).

local Swipe = {}

-- Enter Play with the bursts off and a fresh game; returns the game's script.
function Swipe.start()
    local id = Scene.FindEntityByName("Game")
    Harness.Expect(id ~= nil, "the fruit scene has a Game")
    Harness.Step(1) -- enter Play: the game starts
    local game = Scene.GetScript(id, "fruit")
    Harness.Expect(game ~= nil, "the Game runs fruit.lua")
    game.auto = false
    game:Restart()
    return game
end

-- Step until body `id` is at the top of its arc (its velocity turns downward).
function Swipe.apex(id)
    for _ = 1, 240 do
        local _, vy = Physics.GetVelocity(id)
        if vy <= 0 then return end
        Harness.Step(1)
    end
end

-- Drag the left button across body `id` on screen, `half` pixels either side of
-- it, at a heading of (`dx`, `dy`) (screen, y-up), as a player's mouse would:
-- press, one tick, move, one tick, release.
function Swipe.across(id, half, dx, dy)
    local x, y, z = Transform.GetPosition(id)
    local sx, sy = Camera.WorldToScreen(x, y, z)
    local _, h = UI.GetScreenSize()
    dx, dy = dx or 1, dy or 0
    Input.MoveMouse(sx - dx * half, h - (sy - dy * half)) -- Input is top-left
    Input.Press("MOUSE0")
    Harness.Step(1)
    Input.MoveMouse(sx + dx * half, h - (sy + dy * half))
    Harness.Step(1)
    Input.Release("MOUSE0")
    Harness.Step(1)
end

return Swipe
