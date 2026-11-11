-- scenarios/waves.lua — left to itself the game throws bursts of fruit (and, now
-- and then, a bomb) up into view, and every uncut fruit that falls costs a life.

local id = Scene.FindEntityByName("Game")
Harness.Step(1) -- enter Play: the game starts with its bursts on
local game = Scene.GetScript(id, "fruit")
Harness.Expect(game ~= nil and game.auto, "the Game runs fruit.lua, throwing on its own")

Harness.Step(110) -- the first burst, near the top of its arc
Harness.Expect(game.waves == 1, "one burst thrown, got " .. game.waves)
Harness.Screenshot("cache/test/waves/burst.png", { settle = 8 })

Harness.Step(400)
Harness.Log(string.format("waves %d, missed %d, lives %d, over %s",
    game.waves, game.missed, game.lives, tostring(game.over)))
Harness.Expect(game.missed >= 1, "fruit left alone was missed")
Harness.Expect(game.lives == 3 - game.missed, "each miss cost one life")
