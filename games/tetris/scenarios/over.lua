-- scenarios/over.lua — no inputs until the well fills: the pieces stack in the
-- middle, the stack tops out and game over is reached and shown. The level is
-- raised so the pieces fall a row a tick and the run stays short.

local game_id = Scene.FindEntityByName("Game")
Harness.Step(1)
local game = Scene.GetScript(game_id, "tetris")
game.level = 20

local ticks = 0
while not game.over and ticks < 3000 do
    Harness.Step(10)
    ticks = ticks + 10
end
Harness.Log(string.format("over after %d ticks, %d pieces", ticks, game.pieces))
Harness.Expect(game.over, "the game ends when the stack tops out")
Harness.Expect(game.pieces >= 5, "several pieces fell first, got " .. game.pieces)
Harness.Expect(game.lines == 0, "no line was cleared without input")

Harness.Step(1)
local banner = Scene.FindChild(Scene.FindEntityByName("HUD"), "GameOver")
Harness.Expect(Scene.IsActiveInHierarchy(banner), "the game-over banner is shown")
Harness.Screenshot("cache/test/over/game_over.png", { settle = 8 })

-- The game is over: nothing falls any more, and Enter starts a new one.
local pieces = game.pieces
Harness.Step(60)
Harness.Expect(game.pieces == pieces, "no piece spawns after game over")
Input.Press("ENTER")
Harness.Step(1)
Input.Release("ENTER")
Harness.Step(1)
Harness.Expect(not game.over and game.pieces == 1, "Enter starts a new game")
Harness.Expect(not Scene.IsActiveInHierarchy(banner), "the banner is hidden again")
