-- shots.lua — the fixed cameras the fruit game is screenshotted from at the start:
-- the player's view of the empty kitchen, and the counter up close. The game's own
-- moments (a burst, a cut, game over) are its scenarios' shots, in cache/test/.

return {
    { name = "overview", position = { 0, 2, 13 }, target = { 0, 2, 0 } },
    { name = "counter", position = { 4, -2, 6 }, target = { 0, -4, -1 } },
}
