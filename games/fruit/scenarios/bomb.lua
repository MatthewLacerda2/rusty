-- scenarios/bomb.lua — a swipe across a bomb ends the game: the bomb is gone in a
-- blast, the game is over, its banner shows, and the bursts stop.

local Swipe = dofile("scenarios/lib/swipe.lua")
local game = Swipe.start()

local bomb = game:Throw("bomb", 1, 0, 14)
local orange = game:Throw("orange", -4, 0, 14)
Swipe.apex(bomb)
Harness.Expect(not game.over, "the game runs before the swipe")
Swipe.across(bomb, 150, 1, -0.2)
Harness.Expect(game.over, "cutting the bomb ends the game")
Harness.Expect(Scene.FindEntityByName("bomb") == nil, "the bomb is gone")
Harness.Expect(Scene.FindEntityByName("orange") == orange, "the orange it missed is still there")
Harness.Expect(game.sliced == 0 and game.score == 0, "a bomb scores nothing")
Harness.Step(20)
Harness.Expect(Scene.IsActiveInHierarchy(Scene.FindChild(Scene.FindEntityByName("HUD"), "GameOver")),
    "the game-over banner shows")

-- Over, a swipe cuts nothing, the bursts stay stopped, and a dropped fruit costs no life.
game.auto = true
Swipe.across(orange, 150, 1, 0)
Harness.Expect(game.sliced == 0, "a swipe after game over cuts nothing")
Harness.Step(180)
Harness.Expect(game.waves == 0, "no burst is thrown after game over")
Harness.Expect(game.lives == 3, "a fruit dropped after game over costs no life")
Harness.Screenshot("cache/test/bomb/game_over.png", { settle = 8 })

-- A click once game over has held restarts.
Input.Press("MOUSE0")
Harness.Step(1)
Input.Release("MOUSE0")
Harness.Step(1)
Harness.Expect(not game.over and game.lives == 3, "a click restarts the game")
