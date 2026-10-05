-- tests/fixtures/project/scenarios/bench/enemy.lua — a bench soldier's brain (#835).
--
-- The per-tick work a shooter enemy does, without the combat: it patrols the yard,
-- re-picking a point every couple of seconds (seeded `math.random`, so every run
-- walks the same paths), and checks its line of sight to the Player every tick with
-- a raycast from its eyes, the way perception does.

local Soldier = {}
local state = {} -- per soldier, by entity id
local EYES = 1.6

local function patrol(id)
    NavMeshAgent.SetTarget(id, math.random(-13, 13), 0, math.random(-20, 20))
end

function Soldier.Start(id)
    Animator.Play(id, "Walk")
    state[id] = { repath = math.random(30, 150), sees_player = false }
    patrol(id)
end

function Soldier.Update(id, dt)
    local s = state[id]
    if s == nil then
        return
    end
    s.repath = s.repath - 1
    if s.repath <= 0 then
        patrol(id)
        s.repath = math.random(90, 180)
    end
    local player = Scene.FindEntityByName("Player")
    if player == nil or player == 0 then
        return
    end
    local x, y, z = Transform.GetPosition(id)
    local px, py, pz = Transform.GetPosition(player)
    local hit, other = Physics.Raycast(x, y + EYES, z, px - x, py - y - EYES, pz - z, id)
    s.sees_player = hit and other == player
end

return Soldier
