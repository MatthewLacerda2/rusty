-- scenarios/miss.lua — fruit left alone rises, falls away and costs a life each;
-- the last life lost ends the game.

local Swipe = dofile("scenarios/lib/swipe.lua")
local game = Swipe.start()

local lime = game:Throw("lime", -2, 0.5, 14)
local banana = game:Throw("banana", 2, -0.5, 13)
Swipe.apex(lime)
Harness.Expect(game.lives == 3, "nothing is lost while the fruit is in the air")
Harness.Step(150)
Harness.Expect(Scene.FindEntityByName("lime") == nil and Scene.FindEntityByName("banana") == nil,
    "the fallen fruit despawned")
Harness.Expect(game.missed == 2 and game.lives == 1, "each one cost a life, lives " .. game.lives)
Harness.Expect(not game.over, "a life is left")

game:Throw("melon", 0, 0, 14)
Harness.Step(240)
Harness.Expect(game.missed == 3 and game.lives == 0, "the third cost the last life")
Harness.Expect(game.over, "losing the last life ends the game")
