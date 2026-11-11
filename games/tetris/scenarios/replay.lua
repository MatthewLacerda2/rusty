-- scenarios/replay.lua — determinism: the same seed and inputs, played twice
-- from a restart, end with the same board, score and piece order.

local game_id = Scene.FindEntityByName("Game")
Harness.Step(1)
local game = Scene.GetScript(game_id, "tetris")

local function hold(key, ticks)
    Input.Press(key)
    Harness.Step(ticks)
    Input.Release(key)
    Harness.Step(1)
end

-- Twelve pieces, each turned, moved (tapped, or held past the repeat delay) and
-- soft- or hard-dropped by a fixed pattern.
local function play()
    for i = 1, 12 do
        for _ = 1, i % 4 do hold("UP", 1) end
        hold(i % 2 == 0 and "LEFT" or "RIGHT", (i % 3) * 8 + 1)
        if i % 3 == 0 then hold("DOWN", 20) end
        hold("SPACE", 1)
    end
    Harness.Step(30)
    return game.board, game.score, game.pieces, game.next
end

local function key(board)
    local rows = {}
    for r, line in ipairs(board) do rows[r] = table.concat(line, "") end
    return table.concat(rows, "/")
end

-- Every locked cell has its cube, drawn at its cell: x = col - 5.5, y = row - 0.5.
local function drawn(board)
    for r, line in ipairs(board) do
        for c, v in ipairs(line) do
            local id = game:CubeAt(r, c)
            if (v ~= 0) ~= (id ~= nil) then return false end
            if id then
                local x, y = Transform.GetPosition(id)
                if math.abs(x - (c - 5.5)) > 1e-3 or math.abs(y - (r - 0.5)) > 1e-3 then return false end
            end
        end
    end
    return true
end

local b1, s1, p1, n1 = play()
Harness.Expect(drawn(b1), "every locked cube is drawn at its cell")
game:Restart()
Harness.Step(1)
local b2, s2, p2, n2 = play()

local filled = 0
for _, line in ipairs(b1) do
    for _, v in ipairs(line) do if v ~= 0 then filled = filled + 1 end end
end
Harness.Log(string.format("run 1: score %d, %d pieces, %d cells; run 2: score %d, %d pieces",
    s1, p1, filled, s2, p2))
Harness.Expect(filled >= 20 and p1 >= 12, "the inputs placed pieces on the board")
Harness.Expect(key(b1) == key(b2), "both runs end with the same board")
Harness.Expect(s1 == s2 and p1 == p2 and n1 == n2, "both runs end with the same score and next piece")
