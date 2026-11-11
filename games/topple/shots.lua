-- shots.lua — the fixed cameras the topple game is screenshotted from, on its
-- startup scene (level 1, untouched). The scenarios take the rest from the
-- game's own camera: the barrel going off (boom.lua), a figure mid-fall
-- (replay.lua) and the large pile at rest (settle.lua).

return {
    { name = "tower", position = { 2.5, 2.5, -5 }, target = { 0, 1.8, 0 } },
    { name = "overview", position = { 0, 4, -9 }, target = { 0, 1, 0 } },
}
