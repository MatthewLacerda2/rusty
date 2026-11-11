-- scenarios/shot.lua — a shot goes through the thin wooden wall and hurts the
-- bot behind it, weakened; the same shot into a 40 cm wall hurts no one.
-- Fired with combat.lua's gun, its spread and recoil at zero.

local Combat = require("combat")

Harness.Step(1) -- enter Play: the scripts start
local match = Scene.GetScript(Scene.FindEntityByName("Match"), "match")
match.over = true -- every fighter stands still from here
local player = Scene.FindEntityByName("Player")
local target = Scene.FindEntityByName("Bot_1")
local bot = Scene.GetScript(target, "snowman")

local function shoot(sx, sz, tx, tz)
    Transform.SetPosition(player, sx, 0, sz)
    NavMeshAgent.Warp(target, tx, 0, tz)
    Harness.Step(2) -- the bodies follow their transforms
    bot.health = Combat.HEALTH
    local gun = Combat.gun({ rate = 0, spread = 0, kick = 0, recover = 0 })
    local dx, dz = tx - sx, tz - sz
    local len = math.sqrt(dx * dx + dz * dz)
    Combat.fire(gun, player, sx, Combat.EYE, sz, dx / len, 0, dz / len)
    return Combat.HEALTH - bot.health
end

-- The thin wall stands at x = 6 across z 4.5..7.5 in room A.
local through = shoot(3, 6, 8, 6)
Harness.Log(string.format("through the thin wall: %.1f damage", through))
Harness.Expect(through > 0, "a shot through the thin wall damages the bot behind it")
Harness.Expect(through < 20, "the thin wall takes some of the shot's power")

-- The wall between rooms A and B stands at x = 12, 40 cm thick.
local blocked = shoot(10, 2, 14, 2)
Harness.Log(string.format("through a 40 cm wall: %.1f damage", blocked))
Harness.Expect(blocked == 0, "a 40 cm wall stops the shot")
