-- scenarios/slice.lua — a swipe of the mouse across a thrown apple cuts it: the
-- apple is gone, two halves fall away from where it was, and it scores.

local Swipe = dofile("scenarios/lib/swipe.lua")
local game = Swipe.start()

local apple = game:Throw("apple", 0, 0, 14)
Swipe.apex(apple)
local x, y = Transform.GetPosition(apple)
Harness.Expect(y > 0, string.format("the apple rose into view, y %.2f", y))

-- A swipe that passes beside it cuts nothing.
local _, h = UI.GetScreenSize()
local sx, sy = Camera.WorldToScreen(x, y, 0)
Input.MoveMouse(sx - 200, h - (sy + 150))
Input.Press("MOUSE0")
Harness.Step(1)
Input.MoveMouse(sx + 200, h - (sy + 150))
Harness.Step(1)
Input.Release("MOUSE0")
Harness.Step(1)
Harness.Expect(game.sliced == 0 and game.score == 0, "a swipe above the apple misses it")

Swipe.across(apple, 150, 1, 0.3)
Harness.Expect(game.sliced == 1, "the swipe cut the apple, sliced " .. game.sliced)
Harness.Expect(game.score == 1, "a cut scores 1, got " .. game.score)
Harness.Expect(Scene.FindEntityByName("apple") == nil, "the whole apple is gone")
local halves = game:Halves("apple")
Harness.Expect(#halves == 2, "two halves replace it, got " .. #halves)
Harness.Screenshot("cache/test/slice/split.png", { settle = 8 })

-- The halves part across the swipe and fall.
local ax, ay = Transform.GetPosition(halves[1])
local bx, by = Transform.GetPosition(halves[2])
Harness.Step(30)
local ax2, ay2 = Transform.GetPosition(halves[1])
local bx2, by2 = Transform.GetPosition(halves[2])
local before = math.sqrt((ax - bx) ^ 2 + (ay - by) ^ 2)
local after = math.sqrt((ax2 - bx2) ^ 2 + (ay2 - by2) ^ 2)
Harness.Log(string.format("halves apart %.2f -> %.2f m; fell %.2f m", before, after, ay - ay2))
Harness.Expect(after > before + 0.5, "the halves part")
Harness.Expect(ay2 < ay - 0.5 and by2 < by - 0.5, "the halves fall")

-- Uncut halves falling away cost no life.
Harness.Step(180)
Harness.Expect(#game:Halves("apple") == 0, "the halves despawn below the view")
Harness.Expect(game.lives == 3 and game.missed == 0, "a cut fruit's halves cost no life")
