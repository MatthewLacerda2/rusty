-- shots.lua — the fixed cameras the tetris game is screenshotted from, in edit
-- mode: the empty well and its HUD. The scenarios take the shots of play itself
-- (the first piece, a line about to clear, game over) into their own folders.

return {
    { name = "well", position = { -4.5, 11.5, 32 }, target = { -4.5, 11.5, 0 } },
    { name = "room", position = { 12, 16, 22 }, target = { 0, 8, 0 } },
}
