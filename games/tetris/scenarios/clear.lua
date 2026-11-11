-- scenarios/clear.lua — scripted inputs clear a line: the bottom row is staged
-- full but for four cells one column right of where an I spawns, the I is moved
-- right once and hard-dropped, and the row is gone and scored.

local game_id = Scene.FindEntityByName("Game")
Harness.Expect(game_id ~= nil, "the tetris scene has a Game")
Harness.Step(1) -- enter Play: the game starts and spawns its first piece
local game = Scene.GetScript(game_id, "tetris")
Harness.Expect(game ~= nil, "the Game runs tetris.lua")
Harness.Expect(game.pieces == 1 and not game.over, "the first piece spawned")
Harness.Screenshot("cache/test/clear/start.png", { settle = 8 })

local function tap(key)
    Input.Press(key)
    Harness.Step(1)
    Input.Release(key)
    Harness.Step(1)
end

-- The I spawns flat across columns 4..7; the gap is 5..8, so one step right fills it.
game:Stage({ "XXXX....XX", "X........." }, "I")
Harness.Step(1)
Harness.Expect(game.board[1][5] == 0 and game.board[2][1] == 8, "the position is staged")
tap("RIGHT")
Harness.Screenshot("cache/test/clear/about_to_clear.png", { settle = 8 })
local before = game.score
tap("SPACE")

local board = game.board
Harness.Expect(game.lines == 1, "one line cleared, got " .. game.lines)
local filled = { 0, 0 }
for r = 1, 2 do
    for c = 1, 10 do if board[r][c] ~= 0 then filled[r] = filled[r] + 1 end end
end
Harness.Expect(board[1][1] == 8 and filled[1] == 1, "the row above dropped into the cleared one")
Harness.Expect(filled[2] == 0, "nothing is left on row 2, got " .. filled[2])
-- 100 for the line at level 1, plus 2 a row for the hard drop from row 21 to row 1.
Harness.Expect(game.score - before == 140, "the clear scored, " .. before .. " -> " .. game.score)
Harness.Log(string.format("score %d -> %d, lines %d", before, game.score, game.lines))

-- The cleared cubes burst out of the well as rigidbodies, then despawn.
Harness.Step(20)
Harness.Screenshot("cache/test/clear/burst.png", { settle = 8 })
Harness.Step(90)
Harness.Expect(Scene.FindEntityByName("Cell") ~= nil, "the board's cubes are still there")
